expand:
	cargo expand --lib -p mini_udp --tests
sync-lib-rs-to-readme:
	# #!/bin/sh
	# if [[ -e ".readme.md.tmp" ]]; then
	# 	echo "tmp file .readme.md.tmp already exists, delete it before retrying"
	# 	exit 1
	# fi
	# touch .readme.md.tmp
	# head -n 6 README.md >> .readme.md.tmp
	# sed -e '/\/\/\!/!q' -e 's/\/\/\! //' -e 's/\/\/\!//' mini_udp/src/lib.rs >> .readme.md.tmp
	# tail -n 15 README.md >> .readme.md.tmp
	# cat .readme.md.tmp >| README.md
	# rm .readme.md.tmp
	cargo rdme -w mini_udp
