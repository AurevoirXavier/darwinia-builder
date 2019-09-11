extern crate clap;
extern crate colored;
#[macro_use]
extern crate lazy_static;

// --- std ---
use std::{
	env, fmt, io,
	path::Path,
	process::{Command, Stdio},
};
// --- external ---
use clap::{App, Arg, ArgMatches, SubCommand};
use colored::Colorize;

const STABLE_TOOLCHAIN_VERSION: &'static str = "2019-07-14";

lazy_static! {
	static ref HOST: String = {
		let arch = if cfg!(target_arch = "x86") {
			Arch::x86
		} else if cfg!(target_arch = "x86_64") {
			Arch::x86_64
		} else {
			unreachable!("not support arch")
		};
		let os = if cfg!(target_os = "linux") {
			OS::Linux
		} else if cfg!(target_os = "macos") {
			OS::macOS
		} else if cfg!(target_os = "windows") {
			OS::Windows
		} else {
			unreachable!("not support os")
		};

		format!("{}-{}", arch, os)
	};
	static ref APP: ArgMatches<'static> = App::new("darwinia-builder")
		.author("Xavier Lau <c.estlavie@icloud.com>")
		.about("build tool for darwinia")
		.version("1.0")
		.arg(
			Arg::with_name("host")
				.help("the HOST to build")
				.possible_values(&["x86-windows", "x86_64-windows"])
		)
		.arg(
			Arg::with_name("target")
				.help("the TARGET to run")
				.long("target")
				.value_name("TARGET")
				.possible_values(&[
					"arm-unknown-linux-gnueabi",
					"armv7-unknown-linux-gnueabihf",
					"i686-apple-darwin",
					"x86_64-apple-darwin",
					"i686-unknown-linux-gnu",
					"x86_64-unknown-linux-gnu",
					"i686-pc-windows-msvc",
					"x86_64-pc-windows-msvc",
				])
		)
		.subcommand(
			SubCommand::with_name("build")
				.author("Xavier Lau <c.estlavie@icloud.com>")
				.about("build darwinia")
				.version("1.0")
				.arg(Arg::with_name("release").help("build in release mode"))
		)
		.get_matches();
}

fn main() {
	let builder = Builder::new();
	println!("{:#?}", builder);
}

#[derive(Debug)]
struct Builder {
	toolchain: Tool,
	env_var: EnvVar,
}

impl Builder {
	fn new() -> Self {
		Self {
			toolchain: Tool::new(),
			env_var: EnvVar::new(),
		}
	}

	//	fn build(self) {}
}

#[allow(non_camel_case_types, unused)]
#[derive(Debug)]
enum Arch {
	arm,
	x86,
	x86_64,
}

impl fmt::Display for Arch {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
		match self {
			Arch::arm => write!(f, "arm"),
			Arch::x86 => write!(f, "i686"),
			Arch::x86_64 => write!(f, "x86_64"),
		}
	}
}

#[allow(non_camel_case_types, unused)]
#[derive(Debug)]
enum OS {
	macOS,
	Linux,
	Windows,
}

impl OS {
	fn new() -> Self {
		OS::macOS
	}
}

impl fmt::Display for OS {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
		match self {
			OS::Linux => write!(f, "unknown-linux-gnu"),
			OS::macOS => write!(f, "apple-darwin"),
			OS::Windows => write!(f, "pc-windows-msvc"),
		}
	}
}

#[derive(Debug)]
struct Tool {
	rustup: String,
	cargo: String,
	toolchain: String,
	rustc: String,
	wasm_target: String,
	run_target: String,
}

impl Tool {
	fn new() -> Self {
		let host = APP.value_of("host").unwrap_or(HOST.as_str());
		let mut tool = Self {
			rustup: String::new(),
			cargo: String::new(),
			toolchain: format!("nightly-{}-{}", STABLE_TOOLCHAIN_VERSION, host),
			rustc: String::new(),
			wasm_target: String::from("wasm32-unknown-unknown"),
			run_target: APP.value_of("target").unwrap_or(host).to_owned(),
		};

		match run(Command::new("rustup").arg("--version")) {
			Ok(version) => {
				tool.rustup = version;
				tool.cargo = run(Command::new("cargo").arg("--version")).unwrap();

				{
					let toolchain_list =
						run(Command::new("rustup").args(&["toolchain", "list"])).unwrap();
					if toolchain_list.contains(&tool.toolchain) {
						println!("{} {}", "[✔] toolchain:".green(), tool.toolchain.cyan());
					} else {
						eprintln!("{} {}", "[✘] toolchain:".red(), tool.toolchain.red());

						run_with_output(Command::new("rustup").args(&[
							"toolchain",
							"install",
							&tool.toolchain,
						]))
						.unwrap();

						println!("{} {}", "[✔] toolchain:".green(), tool.toolchain.cyan());
					}

					tool.rustc = run(Command::new("rustc").arg("--version")).unwrap();
				}

				{
					let target_list = run(Command::new("rustup").args(&[
						"target",
						"list",
						"--toolchain",
						&tool.toolchain,
					]))
					.unwrap();
					let mut wasm_target_installed = false;
					let mut run_target_installed = false;

					for line in target_list.lines() {
						if line.contains("(installed)") || line.contains("(default)") {
							if line.contains(&tool.run_target) {
								run_target_installed = true;
							} else if line.contains(&tool.wasm_target) {
								wasm_target_installed = true;
							}
						}
					}

					for (target, target_installed) in vec![
						(&tool.run_target, run_target_installed),
						(&tool.wasm_target, wasm_target_installed),
					] {
						if target_installed {
							println!("{} {}", "[✔] target:".green(), target.cyan());
						} else {
							eprintln!("{} {}", "[✘] target:".red(), target.red());

							run_with_output(Command::new("rustup").args(&[
								"target",
								"add",
								&target,
								"--toolchain",
								&tool.toolchain,
							]))
							.unwrap();

							println!("{} {}", "[✔] target:".green(), target.cyan());
						}
					}
				}
			}
			Err(e) => {
				if e.kind() == io::ErrorKind::NotFound {
					eprintln!(
						"{} {}",
						"[✘] rustup:".red(),
						"https://www.rust-lang.org/tools/install".red()
					);
				} else {
					panic!("{}", e);
				}
			}
		}

		tool
	}
}

#[derive(Debug)]
struct EnvVar {
	target_cc: String,
	openssl_include_dir: String,
	openssl_lib_dir: String,
	rocksdb_lib_dir: String,
}

impl EnvVar {
	fn new() -> Self {
		let mut dir = env::current_dir().unwrap();
		dir.push("/x86_64-linux");
		println!("{:?}", dir);
		let mut env_var = Self {
			target_cc: String::new(),
			openssl_include_dir: String::new(),
			openssl_lib_dir: String::new(),
			rocksdb_lib_dir: String::new(),
		};

		env_var
	}
}

fn run(command: &mut Command) -> Result<String, io::Error> {
	match command.output() {
		Ok(child) => Ok(String::from_utf8_lossy(&child.stdout).trim().to_owned()),
		Err(e) => Err(e),
	}
}

fn run_with_output(command: &mut Command) -> Result<(), io::Error> {
	command.stdout(Stdio::piped()).spawn()?.wait_with_output()?;
	Ok(())
}
