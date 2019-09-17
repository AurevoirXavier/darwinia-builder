## **darwinia builder**

The goal of this project is to simplify the substrate compiling step. 

**darwinia builder** is a must have tool for the substrate developer who wants to make a cross compile. It's super easy to use and support a lot of Arch/OS (in the **future**).

## setup

1. build from source:
   ```sh
	git clone https://github.com/AurevoirXavier/darwinia-builder.git
	cd darwinia-builder
	# only test on lastest nighly version
	cargo +nighly build --release 
	```
   
2. pre-build release: [https://github.com/AurevoirXavier/darwinia-builder/releases](https://github.com/AurevoirXavier/darwinia-builder/releases)

## usage

macOS example:

```sh
cp target/release/darwinia-builder ~/.local/usr/bin
cd /path/to/substrate-project
darwinia-builder --release --wasm --target=x86_64-unknown-linux-gnu --pack

scp target/x86_64-unknown-linux-gnu-substrate-project.tar.gz root@linux.target.machine:~/
ssh root@linux.target.machine

tar xf x86_64-unknown-linux-gnu-substrate-project.tar.gz
cd x86_64-unknown-linux-gnu-substrate-project
./run
```

## screenshot

![screenshot_1](screenshot_1.png)
![screenshot_2](screenshot_2.png)

## contribute

Any issues and PR are welcome!