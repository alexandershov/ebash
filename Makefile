DOCKER_PLATFORM ?= linux/amd64

run:
	cargo run --color=always --package ebash --bin ebash --profile dev --

build:
	cargo build --color=always --package ebash --bin ebash --profile dev

build-release:
	cargo build --color=always --package ebash --bin ebash --profile release

test:
	cargo test -- --nocapture

upgrade-deps:
	# requires `cargo install cargo-edit`
	cargo upgrade

cross-build-linux:
	cross build --target x86_64-unknown-linux-gnu --color=always --package ebash --bin ebash --profile dev

cross-build-linux-release:
	cross build --target x86_64-unknown-linux-musl --color=always --package ebash --bin ebash --profile release

build-docker-image:
	docker build --platform $(DOCKER_PLATFORM) -t shell-test .

run-docker-image-without-cross-build:
	docker run --platform $(DOCKER_PLATFORM) --rm -it -v $(CURDIR)/target/x86_64-unknown-linux-gnu/debug/:/ebash -v $(HOME)/.config/ebash/ebash.toml:/home/shelltest/.config/ebash/ebash.toml:ro shell-test su - shelltest

run-docker-image: cross-build-linux run-docker-image-without-cross-build
	:
