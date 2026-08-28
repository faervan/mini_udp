expand:
	cargo expand --lib -p mini_udp --tests
ci:
	cargo doc --all-features --config build.warnings=\"deny\"
	cargo doc --no-default-features --config build.warnings=\"deny\"
	cargo clippy --all-targets --all-features --config build.warnings=\"deny\"
	cargo clippy --all-targets --no-default-features --config build.warnings=\"deny\"
	cargo test --all-features --config build.warnings=\"deny\"
	cargo test --no-default-features --config build.warnings=\"deny\"
	cargo rdme -w mini_udp
publish: ci
	#!/bin/sh
	version=$(sed -e '1,/\[workspace.package\]/d' -e '/^version =/q' Cargo.toml | cut -d \" -f 2)
	major=$(echo $version | cut -d . -f 1)
	minor=$(echo $version | cut -d . -f 2)
	patch=$(echo $version | cut -d . -f 3)
	echo "Current version is $major.$minor.$patch"
	echo "You can make a"
	echo "  [1] Major update"
	echo "  [2] Minor update"
	echo "  [3] Patch"
	echo -n "Select [1/2/3] "
	read line
	if [[ $line -eq 1 ]]; then
		next="$((major+1)).0.0"
	elif [[ $line -eq 2 ]]; then
		next="$major.$((minor+1)).0"
	elif [[ $line -eq 3 ]]; then
		next="$major.$minor.$((patch+1))"
	else
		exit 1
	fi
	echo "Next version will be $next"
	echo -n "Is that correct? [y/n] "
	read line
	if [[ $line != "y" && $line != "Y" ]]; then
		exit 1
	fi
	cargo rdme -w mini_udp
	derive_line="mini_udp_derive = { path = \"derive\", version = \"$version\" }"
	grep -q "$derive_line" Cargo.toml || {
		echo -e "\nFailed to find derive dependency line"
		exit 1
	}
	echo -n "Have you included all changes in the CHANGELOG? [y/n] "
	read line
	if [[ $line != "y" && $line != "Y" ]]; then
		exit 1
	fi
	grep "$next" CHANGELOG.md || {
		today=$(date +%Y-%m-%d)
		sed -e "0,/\[Unreleased\]/s//\[Unreleased\]\n\n\#\# \[$next\] - $today/" -i CHANGELOG.md
	}
	sed -e "0,/version = \"$version\"/s/version = \"$version\"/version = \"$next\"/" -i Cargo.toml
	sed -e "0,/$derive_line/s/version = \"$version\"/version = \"$next\"/" -i Cargo.toml
	# Update Cargo.lock
	cargo build
	echo -e "\n\033[32mAll done. You can now commit and publish:\033[0m"
	echo -e "\tgit commit -am \"chore: release v$next\""
	echo -e "\tgit push"
	echo -e "\tcargo publish"
	echo -en "\nDo that? [y/n] "
	read line
	if [[ $line == "y" || $line == "Y" ]]; then
		git commit -am "chore: release v$next"
		git push
		cargo publish
	fi
