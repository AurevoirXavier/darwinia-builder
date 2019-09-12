extern crate clap;
extern crate colored;
#[macro_use]
extern crate lazy_static;

// --- std ---
use std::{
	env, fmt, io,
	process::{Command, Stdio},
};
// --- external ---
use clap::{App, Arg, ArgMatches};
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
				.long("host")
				.value_name("HOST")
				.possible_values(&[
					"i686-apple-darwin",
					"x86_64-apple-darwin",
					"i686-unknown-linux-gnu",
					"x86_64-unknown-linux-gnu",
					"i686-pc-windows-msvc",
					"x86_64-pc-windows-msvc",
				])
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
		.arg(
			Arg::with_name("release")
				.long("release")
				.help("build darwinia in release mode")
		)
		.arg(
			Arg::with_name("wasm")
				.long("wasm")
				.help("also build wasm in release mode")
		)
		.get_matches();
}

fn main() {
	let builder = Builder::new();
	if builder.check() {
		builder.build().unwrap();
	}
}

#[derive(Debug)]
struct Builder {
	tool: Tool,
	env_var: EnvVar,
}

impl Builder {
	fn new() -> Self {
		Self {
			tool: Tool::new(),
			env_var: EnvVar::new(),
		}
	}

	fn check(&self) -> bool {
		let Builder {
			tool: Tool {
				rustup,
				cargo,
				toolchain,
				wasm_target,
				run_target,
			},
			env_var:
				EnvVar {
					target_cc,
					sysroot,
					openssl_include_dir,
					openssl_lib_dir,
					rocksdb_lib_dir,
				},
		} = self;

		![
			rustup,
			cargo,
			toolchain,
			wasm_target,
			run_target,
			target_cc,
			sysroot,
			openssl_include_dir,
			openssl_lib_dir,
			rocksdb_lib_dir,
		]
		.iter()
		.any(|&s| s.is_empty())
	}

	fn build(self) -> Result<(), io::Error> {
		self.build_wasm()?;
		self.build_darwinia()?;

		Ok(())
	}

	fn build_wasm(&self) -> Result<(), io::Error> {
		let root_path = env::current_dir()?;

		{
			let mut wasm_path = root_path.clone();
			wasm_path.push("node/run/time/wasm");
			env::set_current_dir(&wasm_path)?;
		}

		env::set_var("CARGO_INCREMENTAL", "0");

		run_with_output(Command::new("cargo").args(&[
			&format!("+{}", self.tool.toolchain),
			"rustc",
			"--release",
			"--target",
			"wasm32-unknown-unknown",
			"--",
			"-C",
			"link-arg=--export-table",
		]))?;

		run(Command::new("wasm-gc").args(&[
			"target/wasm32-unknown-unknown/release/node_runtime.wasm",
			"target/wasm32-unknown-unknown/release/node_runtime.compact.wasm",
		]))?;

		env::set_current_dir(&root_path)?;

		Ok(())
	}

	fn build_darwinia(&self) -> Result<(), io::Error> {
		let mut build_command = Command::new("cargo");
		build_command.args(&[&format!("+{}", self.tool.toolchain), "rustc"]);

		if APP.is_present("release") {
			build_command.arg("--release");
		}

		if let Some(target) = APP.value_of("target") {
			env::set_var("CARGO_INCREMENTAL", "1");
			env::set_var(
				"TARGET_CC",
				self.env_var.target_cc.splitn(2, ' ').next().unwrap(),
			);
			env::set_var("SYSROOT", &self.env_var.sysroot);
			env::set_var("OPENSSL_INCLUDE_DIR", &self.env_var.openssl_include_dir);
			env::set_var("OPENSSL_LIB_DIR", &self.env_var.openssl_lib_dir);
			env::set_var("ROCKSDB_LIB_DIR", &self.env_var.rocksdb_lib_dir);

			build_command.args(&[
				"--target",
				target,
				"--",
				"-C",
				&format!("link_args=--sysroot={}", self.env_var.sysroot),
			]);
		}

		run_with_output(&mut build_command)?;

		if APP.is_present("target") {
			env::remove_var("TARGET_CC");
			env::remove_var("SYSROOT");
			env::remove_var("OPENSSL_INCLUDE_DIR");
			env::remove_var("OPENSSL_LIB_DIR");
			env::remove_var("ROCKSDB_LIB_DIR");
		}

		Ok(())
	}
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
						println!("{} {}", "[✓] toolchain:".green(), tool.toolchain.cyan());
					} else {
						eprintln!("{} {}", "[✗] toolchain:".red(), tool.toolchain.red());

						run_with_output(Command::new("rustup").args(&[
							"toolchain",
							"install",
							&tool.toolchain,
						]))
						.unwrap();

						println!("{} {}", "[✓] toolchain:".green(), tool.toolchain.cyan());
					}
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
							println!("{} {}", "[✓] target:".green(), target.cyan());
						} else {
							eprintln!("{} {}", "[✗] target:".red(), target.red());

							run_with_output(Command::new("rustup").args(&[
								"target",
								"add",
								&target,
								"--toolchain",
								&tool.toolchain,
							]))
							.unwrap();

							println!("{} {}", "[✓] target:".green(), target.cyan());
						}
					}
				}
			}
			Err(e) => {
				if e.kind() == io::ErrorKind::NotFound {
					eprintln!(
						"{} {}",
						"[✗] rustup:".red(),
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
	sysroot: String,
	openssl_include_dir: String,
	openssl_lib_dir: String,
	rocksdb_lib_dir: String,
}

impl EnvVar {
	fn new() -> Self {
		let host = APP.value_of("host").unwrap_or(HOST.as_str());
		let mut target_cc = String::new();
		let mut sysroot = String::new();
		let mut openssl_include_dir = String::new();
		let mut openssl_lib_dir = String::new();
		let mut rocksdb_lib_dir = String::new();
		let mut dir = env::current_dir().unwrap();

		if APP.value_of("target").is_some() {
			// TODO
			match APP.value_of("target").unwrap_or(host) {
				"arm-unknown-linux-gnueabi" => unimplemented!(),
				"armv7-unknown-linux-gnueabihf" => unimplemented!(),
				"i686-apple-darwin" => unimplemented!(),
				"x86_64-apple-darwin" => unimplemented!(),
				"i686-unknown-linux-gnu" => unimplemented!(),
				"x86_64-unknown-linux-gnu" => {
					match run(Command::new("x86_64-unknown-linux-gnu-gcc").arg("--version")) {
						Ok(version) => {
							target_cc = version.splitn(2, '\n').next().unwrap().to_owned();
							println!(
								"{} {}",
								"[✓] x86_64-unknown-linux-gnu-gcc:".green(),
								target_cc.cyan()
							);
						}
						Err(e) => {
							if e.kind() == io::ErrorKind::NotFound {
								eprintln!(
									"{} {}",
									"[✗] x86_64-unknown-linux-gnu-gcc:".red(),
									"https://github.com/SergioBenitez/homebrew-osxct".red()
								);
							} else {
								panic!("{}", e);
							}
						}
					}

					dir.push("linux-x86_64");

					if dir.exists() {
						println!(
							"{} {}",
							"[✓] linux-x86_64:".green(),
							dir.to_string_lossy().cyan()
						);
					} else {
						eprintln!(
							"{} {}",
							"[✗] linux-x86_64:".red(),
							"https://github.com/AurevoirXavier/darwinia-builder/releases/download/v0.2.0/linux-x86_64.tar.gz".red()
						);
						eprintln!("{}", "[✗] SYSROOT".red(),);
						eprintln!("{}", "[✗] OPENSSL_INCLUDE_DIR".red(),);
						eprintln!("{}", "[✗] OPENSSL_LIB_DIR".red(),);
						eprintln!("{}", "[✗] ROCKSDB_LIB_DIR".red(),);

						return Self {
							target_cc,
							sysroot,
							openssl_include_dir,
							openssl_lib_dir,
							rocksdb_lib_dir,
						};
					}
				}
				"i686-pc-windows-msvc" => unimplemented!(),
				"x86_64-pc-windows-msvc" => unimplemented!(),
				_ => unreachable!(),
			}

			for (k, v, folder) in [
				("SYSROOT", &mut sysroot, "sysroot"),
				("OPENSSL_INCLUDE_DIR", &mut openssl_include_dir, "include"),
				("OPENSSL_LIB_DIR", &mut openssl_lib_dir, "lib/openssl"),
				("ROCKSDB_LIB_DIR", &mut rocksdb_lib_dir, "lib/rocksdb"),
			]
			.iter_mut()
			{
				if let Ok(v_) = env::var(*k) {
					**v = v_;
				} else {
					let mut dir = dir.clone();
					dir.push(*folder);
					**v = dir.to_string_lossy().to_string();
				}
				println!(
					"{} {}{} {}",
					"[✓]".green(),
					(*k).green(),
					":".green(),
					v.cyan()
				);
			}
		}

		Self {
			target_cc,
			sysroot,
			openssl_include_dir,
			openssl_lib_dir,
			rocksdb_lib_dir,
		}
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
