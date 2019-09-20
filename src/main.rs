extern crate clap;
extern crate colored;
extern crate dirs;
extern crate fs_extra;
extern crate indicatif;
#[macro_use]
extern crate lazy_static;
extern crate reqwest;
extern crate toml;

// --- std ---
use std::{
	env, fmt,
	fs::{self, File, OpenOptions},
	io::{self, Read, Write},
	os::unix::fs::OpenOptionsExt,
	path::Path,
	process::{Command, Stdio},
};
// --- external ---
use clap::{App, Arg, ArgMatches};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::{
	header::{CONTENT_LENGTH, RANGE},
	ClientBuilder, Url,
};

const STABLE_TOOLCHAIN_VERSION: &'static str = "2019-07-14";

const RUSTUP: &'static str = "https://www.rust-lang.org/tools/install";
const WASM_GC: &'static str = "https://github.com/alexcrichton/wasm-gc";

const LINUX_86_64_DEPS: &'static str = "https://github.com/AurevoirXavier/darwinia-builder/releases/download/linux-x86_64/linux-x86_64.tar.gz";
const WINDOWS_86_64_DEPS: &'static str = "https://github.com/AurevoirXavier/darwinia-builder/releases/download/windows-x86_64/windows-x86_64.tar.gz";

lazy_static! {
	static ref HOST: String = {
		let arch = if cfg!(target_arch = "x86") {
			Arch::x86
		} else if cfg!(target_arch = "x86_64") {
			Arch::x86_64
		} else {
			unreachable!("unsupported arch")
		};
		let os = if cfg!(target_os = "linux") {
			OS::Linux
		} else if cfg!(target_os = "macos") {
			OS::macOS
		} else if cfg!(target_os = "windows") {
			OS::Windows
		} else {
			unreachable!("unsupported os")
		};

		format!("{}-{}", arch, os)
	};
	static ref APP: ArgMatches<'static> =
		App::new("darwinia-builder")
			.author("Xavier Lau <c.estlavie@icloud.com>")
			.about("build tool for darwinia")
			.version("0.7.2-alpha")
			.arg(
				Arg::with_name("host")
					.help("The HOST to build")
					.long("host")
					.value_name("HOST")
					.possible_values(&[
						"i686-apple-darwin",
						"x86_64-apple-darwin",
						"i686-unknown-linux-gnu",
						"x86_64-unknown-linux-gnu",
						"i686-pc-windows-gnu",
						"x86_64-pc-windows-gnu",
					])
			)
			.arg(
				Arg::with_name("target")
					.help("The TARGET to run")
					.long("target")
					.value_name("TARGET")
					.possible_values(&[
						"arm-unknown-linux-gnueabi",
						"armv7-unknown-linux-gnueabihf",
						"i686-apple-darwin",
						"x86_64-apple-darwin",
						"i686-unknown-linux-gnu",
						"x86_64-unknown-linux-gnu",
						"i686-pc-windows-gnu",
						"x86_64-pc-windows-gnu",
					])
			)
			.arg(
				Arg::with_name("release")
					.long("release")
					.help("Build darwinia in release mode")
			)
			.arg(
				Arg::with_name("wasm")
					.long("wasm")
					.help("Also build wasm in release mode")
			)
			.arg(
				Arg::with_name("pack")
					.long("pack")
					.help("Pack darwinia and LD_LIBRARY into darwinia.tar.gz (ONLY pack for UNIX)")
			)
			.arg(Arg::with_name("verbose").long("verbose").help(
				"Use verbose output (-vv very verbose/build.rs output) while building darwinia"
			))
			.get_matches();
}

fn main() {
	let builder = Builder::new();
	if builder.check() {
		builder.build().unwrap();
		if APP.is_present("pack") {
			builder.pack().unwrap();
		}
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
			tool:
				Tool {
					rustup,
					cargo,
					toolchain,
					run_target,
					wasm_target,
					wasm_gc,
				},
			env_var:
				EnvVar {
					config_file,
					target_cc,
					deps,
					sysroot,
					openssl_include_dir,
					openssl_lib_dir,
					rocksdb_lib_dir,
				},
		} = self;

		let essential_check = ![rustup, cargo, toolchain, wasm_target, wasm_gc, run_target]
			.iter()
			.any(|&s| s.is_empty());
		if APP.is_present("target") {
			// we use rust-native-tls,
			// which will use the operating system TLS framework if available, meaning Windows and macOS.
			// On Linux, it will use OpenSSL 1.1.
			let cross_compile_check = if run_target.contains("linux") {
				![
					config_file,
					target_cc,
					deps,
					sysroot,
					openssl_include_dir,
					openssl_lib_dir,
					rocksdb_lib_dir,
				]
				.iter()
				.any(|&s| s.is_empty())
			} else {
				![config_file, target_cc, deps, rocksdb_lib_dir]
					.iter()
					.any(|&s| s.is_empty())
			};

			essential_check && cross_compile_check
		} else {
			essential_check
		}
	}

	fn build(&self) -> Result<(), io::Error> {
		if APP.is_present("wasm") {
			self.build_wasm()?;
		}
		self.build_darwinia()?;

		Ok(())
	}

	fn pack(&self) -> Result<(), io::Error> {
		let is_windows = self.tool.run_target.contains("windows");
		let root_path = env::current_dir()?;
		let target_dir = {
			let mut p = root_path.clone();
			p.push("target");

			p
		};
		let package_name = {
			let mut cargo_toml = fs::File::open("Cargo.toml")?;
			let mut config = String::new();
			cargo_toml.read_to_string(&mut config)?;

			let config = config.parse::<toml::Value>().unwrap();
			let config = config.as_table().unwrap();
			let package = config.get("package").unwrap();
			let package = package.as_table().unwrap();

			package.get("name").unwrap().as_str().unwrap().to_owned()
		};

		let mut target_path = target_dir.clone();
		target_path.push(&self.tool.run_target);
		if APP.is_present("release") {
			target_path.push("release");
		} else {
			target_path.push("debug");
		}
		if is_windows {
			target_path.push(&format!("{}.exe", &package_name));
		} else {
			target_path.push(&package_name);
		}

		let mut ld_library_dir = root_path.clone();
		ld_library_dir.push(&self.env_var.deps);
		ld_library_dir.push("ld-library");

		let mut pack_path = target_dir.clone();
		pack_path.push(&format!("{}-{}", self.tool.run_target, package_name));
		let pack_dir = pack_path.clone();
		if !pack_path.is_dir() {
			fs::create_dir(&pack_path)?;
		}

		{
			if is_windows {
				pack_path.push(&format!("{}.exe", &package_name));
			} else {
				pack_path.push(&package_name);
			}
			fs::copy(&target_path, &pack_path)?;

			let mut copy_options = fs_extra::dir::CopyOptions::new();
			copy_options.overwrite = true;
			fs_extra::dir::copy(&ld_library_dir, &pack_dir, &copy_options).unwrap();

			drop(target_path);
			drop(ld_library_dir);
		}

		if !is_windows {
			let mut run_script = fs::OpenOptions::new()
				.create(true)
				.truncate(true)
				.write(true)
				.mode(0o755)
				.open(&format!("{}/run.sh", pack_dir.to_string_lossy()))?;
			run_script.write(
				format!(
					"#!/usr/bin/env bash\nexport LD_LIBRARY_PATH=$LD_LIBRARY:$(pwd)/ld-library\n./{}",
					&package_name
				)
				.as_bytes(),
			)?;
			run_script.sync_all()?;
		}

		env::set_current_dir(&target_dir)?;
		run(Command::new("tar").args(&[
			"zcf",
			&format!("{}-{}.tar.gz", self.tool.run_target, &package_name),
			&format!("{}-{}", self.tool.run_target, package_name),
		]))?;
		env::set_current_dir(&root_path)?;

		Ok(())
	}

	fn build_wasm(&self) -> Result<(), io::Error> {
		let root_path = env::current_dir()?;

		{
			let mut wasm_path = root_path.clone();
			wasm_path.push("node/runtime/wasm");
			env::set_current_dir(&wasm_path)?;
		}

		run_with_output(
			Command::new("cargo")
				.args(&[
					&format!("+{}", self.tool.toolchain),
					"rustc",
					"--release",
					"--target",
					"wasm32-unknown-unknown",
					"--",
					"-C",
					"link-arg=--export-table",
				])
				.env("CARGO_INCREMENTAL", "0"),
		)?;
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
		if APP.is_present("verbose") {
			build_command.arg("--verbose");
		}
		if let Some(target) = APP.value_of("target") {
			build_command.env("CARGO_INCREMENTAL", "1");
			build_command.env(
				"TARGET_CC",
				self.env_var.target_cc.splitn(2, ' ').next().unwrap(),
			);
			build_command.env("SYSROOT", &self.env_var.sysroot);
			build_command.env("OPENSSL_INCLUDE_DIR", &self.env_var.openssl_include_dir);
			build_command.env("OPENSSL_LIB_DIR", &self.env_var.openssl_lib_dir);
			build_command.env("ROCKSDB_LIB_DIR", &self.env_var.rocksdb_lib_dir);

			build_command.args(&[
				"--target",
				target,
				"--",
				"-C",
				&format!("link_args=--sysroot={}", self.env_var.sysroot),
			]);
		}

		run_with_output(&mut build_command)?;

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
			OS::Windows => write!(f, "pc-windows-gnu"),
		}
	}
}

#[derive(Debug)]
struct Tool {
	rustup: String,
	cargo: String,
	toolchain: String,
	run_target: String,
	wasm_target: String,
	wasm_gc: String,
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
			wasm_gc: String::new(),
		};

		match run(Command::new("rustup").arg("--version")) {
			Ok(version) => tool.rustup = version,
			Err(e) => {
				if e.kind() == io::ErrorKind::NotFound {
					// TODO
					// if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
					// run_with_output(&mut Command::new(
					// "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh",
					// ))
					// .unwrap();
					// } else {
					eprintln!("{} {}", "[✗] rustup:".red(), RUSTUP.red());
				// }
				} else {
					panic!("{}", e);
				}
			}
		}
		println!("{} {}", "[✓] rustup:".green(), tool.rustup.cyan());

		tool.cargo = run(Command::new("cargo").arg("--version")).unwrap();
		println!("{} {}", "[✓] cargo:".green(), tool.cargo.cyan());

		{
			let toolchain_list = run(Command::new("rustup").args(&["toolchain", "list"])).unwrap();
			if !toolchain_list.contains(&tool.toolchain) {
				eprintln!("{} {}", "[✗] toolchain:".red(), tool.toolchain.red());

				run_with_output(Command::new("rustup").args(&[
					"toolchain",
					"install",
					&tool.toolchain,
				]))
				.unwrap();
			}

			println!("{} {}", "[✓] toolchain:".green(), tool.toolchain.cyan());
		}

		{
			let target_list = run(Command::new("rustup").args(&[
				"target",
				"list",
				"--toolchain",
				&tool.toolchain,
			]))
			.unwrap();
			let mut run_target_installed = false;
			let mut wasm_target_installed = false;

			for line in target_list.lines() {
				if line.contains("(installed)") || line.contains("(default)") {
					if line.contains(&tool.run_target) {
						run_target_installed = true;
					} else if line.contains(&tool.wasm_target) {
						wasm_target_installed = true;
					}
				}
			}

			for (target, target_installed) in [
				(&tool.run_target, run_target_installed),
				(&tool.wasm_target, wasm_target_installed),
			]
			.iter()
			{
				if !target_installed {
					eprintln!("{} {}", "[✗] target:".red(), target.red());

					run_with_output(Command::new("rustup").args(&[
						"target",
						"add",
						target,
						"--toolchain",
						&tool.toolchain,
					]))
					.unwrap();
				}

				println!("{} {}", "[✓] target:".green(), target.cyan());
			}
		}

		{
			if let Err(e) = run(Command::new("wasm-gc").arg("--help")) {
				if e.kind() == io::ErrorKind::NotFound {
					eprintln!("{} {}", "[✗] wasm-gc:".red(), WASM_GC.red());

					run_with_output(Command::new("cargo").args(&["install", "--git", WASM_GC]))
						.unwrap();
				} else {
					panic!("{}", e);
				}
			}

			tool.wasm_gc = String::from(WASM_GC);
			println!("{} {}", "[✓] wasm-gc:".green(), tool.wasm_gc.cyan());
		}

		tool
	}
}

#[derive(Debug)]
struct EnvVar {
	config_file: String,
	target_cc: String,
	deps: String,
	sysroot: String,
	openssl_include_dir: String,
	openssl_lib_dir: String,
	rocksdb_lib_dir: String,
}

impl EnvVar {
	fn new() -> Self {
		let mut config_file = String::new();
		let mut target_cc = String::new();
		let mut deps = String::new();
		let mut sysroot = String::new();
		let mut openssl_include_dir = String::new();
		let mut openssl_lib_dir = String::new();
		let mut rocksdb_lib_dir = String::new();
		let mut dir = env::current_dir().unwrap();

		if !APP.is_present("target") {
			return Self {
				config_file,
				target_cc,
				deps,
				sysroot,
				openssl_include_dir,
				openssl_lib_dir,
				rocksdb_lib_dir,
			};
		}

		let (mut config_file_handler, config) = {
			let mut config_file_path = dirs::home_dir().unwrap();
			config_file_path.push(".cargo");
			if !config_file_path.as_path().is_dir() {
				fs::create_dir(&config_file_path).unwrap();
			}

			config_file_path.push("config");
			if config_file_path.is_file() {
				let mut config_file_handler = OpenOptions::new()
					.read(true)
					.write(true)
					.open(&config_file_path)
					.unwrap();
				let mut config = String::new();
				config_file_handler.read_to_string(&mut config).unwrap();
				(config_file_handler, config)
			} else {
				(File::create(&config_file_path).unwrap(), String::new())
			}
		};

		// TODO
		let target = APP.value_of("target").unwrap();
		match target {
			"arm-unknown-linux-gnueabi" => unimplemented!(),
			"armv7-unknown-linux-gnueabihf" => unimplemented!(),
			"i686-apple-darwin" => unimplemented!(),
			"x86_64-apple-darwin" => unimplemented!(),
			"i686-unknown-linux-gnu" => unimplemented!(),
			"x86_64-unknown-linux-gnu" => {
				match run(Command::new("x86_64-unknown-linux-gnu-gcc").arg("--version")) {
					Ok(version) => {
						// TODO
						target_cc = version.splitn(2, '\n').next().unwrap().to_owned();
						config_file = format!(
							"[target.x86_64-unknown-linux-gnu]\nlinker = \"{}\"",
							target_cc.splitn(2, ' ').next().unwrap()
						);

						println!(
							"{} {}",
							"[✓] x86_64-unknown-linux-gnu-gcc:".green(),
							target_cc.cyan()
						);
					}
					Err(e) => {
						if e.kind() == io::ErrorKind::NotFound {
							// TODO
							if cfg!(target_os = "macos") {
								eprintln!(
									"{} {}",
									"[✗] x86_64-unknown-linux-gnu-gcc:".red(),
									"https://github.com/SergioBenitez/homebrew-osxct".red()
								);
							}
						} else {
							panic!("{}", e);
						}
					}
				}

				set_config_file(
					&config,
					&mut config_file,
					&mut config_file_handler,
					"[target.x86_64-unknown-linux-gnu]",
				)
				.unwrap();

				dir.push("linux-x86_64");
				chcek_deps(dir.as_path(), &mut deps, LINUX_86_64_DEPS).unwrap();
			}
			"i686-pc-windows-gnu" => unimplemented!(),
			"x86_64-pc-windows-gnu" => {
				match run(Command::new("x86_64-w64-mingw32-gcc").arg("--version")) {
					Ok(version) => {
						// TODO
						target_cc = version.splitn(2, '\n').next().unwrap().to_owned();
						config_file = format!(
							"[target.x86_64-pc-windows-gnu]\nlinker = \"{}\"",
							target_cc.splitn(2, ' ').next().unwrap()
						);

						println!(
							"{} {}",
							"[✓] x86_64-w64-mingw32-gcc:".green(),
							target_cc.cyan()
						);
					}
					Err(e) => {
						if e.kind() == io::ErrorKind::NotFound {
							// TODO
							if cfg!(target_os = "macos") {
								eprintln!(
									"{} {}",
									"[✗] x86_64-w64-mingw32-gcc:".red(),
									"https://formulae.brew.sh/formula/mingw-w6 and MUST CHECK https://github.com/rust-lang/rust/issues/48272#issuecomment-429596397".red()
								);
							}
						} else {
							panic!("{}", e);
						}
					}
				}

				set_config_file(
					&config,
					&mut config_file,
					&mut config_file_handler,
					"[target.x86_64-pc-windows-gnu]",
				)
				.unwrap();

				dir.push("windows-x86_64");
				chcek_deps(dir.as_path(), &mut deps, WINDOWS_86_64_DEPS).unwrap();
			}
			_ => unreachable!(),
		}

		if target.contains("linux") {
			for (k, v, folder) in [
				("SYSROOT", &mut sysroot, "sysroot"),
				("OPENSSL_INCLUDE_DIR", &mut openssl_include_dir, "include"),
				("OPENSSL_LIB_DIR", &mut openssl_lib_dir, "lib/openssl"),
				("ROCKSDB_LIB_DIR", &mut rocksdb_lib_dir, "lib/rocksdb"),
			]
			.iter_mut()
			{
				check_envs(k, v, dir.as_path(), folder);
			}
		} else {
			check_envs(
				"ROCKSDB_LIB_DIR",
				&mut rocksdb_lib_dir,
				dir.as_path(),
				"lib/rocksdb",
			);
		}

		Self {
			config_file,
			target_cc,
			deps,
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

fn set_config_file(
	config: &str,
	config_file: &str,
	config_file_handler: &mut File,
	target: &str,
) -> Result<(), io::Error> {
	let mut config_unset = true;

	for line in config.lines() {
		if line.trim() == target {
			config_unset = false;
		}
	}

	if config_unset {
		eprintln!(
			"{} {}",
			"[✗] config file:".red(),
			"will be set automatically".red()
		);

		if !config.is_empty() {
			config_file_handler.write("\n\n".as_bytes())?;
		}
		config_file_handler.write(config_file.as_bytes())?;
		config_file_handler.sync_all()?;
	}

	println!(
		"{} {}",
		"[✓] config file:".green(),
		config_file.replace('\n', " ").cyan()
	);

	Ok(())
}

fn chcek_deps(dir: &Path, deps: &mut String, download_link: &str) -> Result<(), io::Error> {
	if !dir.exists() {
		eprintln!(
			"{} {} {}",
			"[✗] deps:".red(),
			"automatically download from:".red(),
			download_link.red(),
		);

		let download_link = Url::parse(download_link).unwrap();
		if let Err(e) = download(&download_link) {
			eprintln!(
				"{} {}",
				"download failed:".red(),
				e.to_string().as_str().red()
			);
		} else {
			run(Command::new("tar")
				.args(&["xf", download_link.path_segments().unwrap().last().unwrap()]))?;

			*deps = dir.to_string_lossy().to_string();
			println!("{} {}", "[✓] deps:".green(), deps.cyan());
		}
	} else {
		*deps = dir.to_string_lossy().to_string();
		println!("{} {}", "[✓] deps:".green(), deps.cyan());
	}

	Ok(())
}

fn check_envs(k: &str, v: &mut String, dir: &Path, folder: &str) {
	if let Ok(v_) = env::var(k) {
		*v = v_;
	} else {
		let mut dir = dir.clone().to_path_buf();
		dir.push(folder);
		if dir.as_path().is_dir() {
			*v = dir.to_string_lossy().to_string();

			println!(
				"{} {}{} {}",
				"[✓]".green(),
				(*k).green(),
				":".green(),
				v.cyan()
			);
		} else {
			eprintln!("{} {}", "[✗]".red(), (*k).red());
		}
	}
}

struct DownloadProgress<R> {
	inner: R,
	progress_bar: ProgressBar,
}

impl<R: Read> Read for DownloadProgress<R> {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		self.inner.read(buf).map(|n| {
			self.progress_bar.inc(n as u64);
			n
		})
	}
}

fn download(url: &Url) -> Result<(), reqwest::Error> {
	let client = ClientBuilder::new()
		.danger_accept_invalid_certs(true)
		.danger_accept_invalid_hostnames(true)
		.gzip(true)
		.use_sys_proxy()
		.build()?;
	let total_size = client
		.get(url.as_str())
		.send()?
		.headers()
		.get(CONTENT_LENGTH)
		.unwrap()
		.to_str()
		.unwrap()
		.parse()
		.unwrap();
	let mut req = client.get(url.as_str());

	let pb = ProgressBar::new(total_size);
	pb.set_style(
		ProgressStyle::default_bar()
			.template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
			.progress_chars("=> ")
	);

	let file = Path::new(url.path_segments().unwrap().last().unwrap());
	if file.exists() {
		let size = file.metadata().unwrap().len() - 1;

		req = req.header(RANGE, format!("bytes={}-", size));
		pb.inc(size);
	}

	let mut source = DownloadProgress {
		progress_bar: pb,
		inner: req.send().unwrap(),
	};
	let mut dest = fs::OpenOptions::new()
		.create(true)
		.append(true)
		.open(&file)
		.unwrap();
	io::copy(&mut source, &mut dest).unwrap();
	dest.sync_all().unwrap();

	Ok(())
}
