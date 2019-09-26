## **Darwinia Builder**

The goal of this project is to simplify the substrate compiling step. 

**darwinia builder** is a must have tool for the substrate developer who wants to make a cross compile. It's super easy to use and support a lot of Arch/OS.

### Cross Compile Support
- [x] from **macOS (x86_64)** to **Linux (x86_64)**
- [ ] from **macOS (x86_64)** to **Linux (x86)**
- [x] from **macOS (x86_64)** to **Windows (x86_64)**
- [ ] from **macOS (x86_64)** to **Windows (x86)**
- [x] from **Linux (x86_64)** to **macOS (x86_64)**
- [x] from **Linux (x86_64)** to **Windows (x86_64)**
- [ ] from **Linux (x86_64)** to **Windows (x86)**
- [ ] from **Windows** to **Linux**
- [ ] from **Windows** to **macOS**

### Tested HOST Version/Distribution
- macOS Mojave 10.14.6
- ArchLinux 5.3.1

## Setup

1. build from source:
   ```sh
	git clone https://github.com/AurevoirXavier/darwinia-builder.git
	cd darwinia-builder
	# only test on lastest nighly version
	cargo +nightly build --release 
	```
   
2. pre-build release: [https://github.com/AurevoirXavier/darwinia-builder/releases](https://github.com/AurevoirXavier/darwinia-builder/releases)

## Usage

from **macOS** to **Linux** example:

```sh
mv /path/to/darwinia-builder ~/.local/usr/bin
cd /path/to/substrate-project
darwinia-builder --release --wasm --target=x86_64-unknown-linux-gnu --pack

scp target/x86_64-unknown-linux-gnu-substrate-project.tar.gz root@linux.target.machine:~/
ssh root@linux.target.machine

tar xf x86_64-unknown-linux-gnu-substrate-project.tar.gz
cd x86_64-unknown-linux-gnu-substrate-project
chmod u+x run.sh
./run.sh
```

## Screenshot

![screenshot_1](screenshot_1.png)
![screenshot_2](screenshot_2.png)

## Contribute

Any issue and PR are welcome!