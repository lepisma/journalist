.PHONY: deploy

deploy:
	scp ./target/release/journalist lepisma@euclid-yellow:/mnt/ssd/applications/journalist/
