.PHONY: build build-frontend build-backend deploy test clean docker-build docker-run install

build: build-frontend build-backend deploy

build-frontend:
	cd frontend && npm ci --no-audit && npm run build

build-backend:
	cd backend && cargo build --release --locked

deploy:
	mkdir -p backend/static
	rm -rf backend/static/*
	cp -r frontend/build/* backend/static/

test:
	cd backend && cargo test --locked
	cd frontend && npm run lint
	cd frontend && npm test -- --watchAll=false

clean:
	rm -rf backend/target frontend/build frontend/node_modules backend/static

docker-build:
	docker build -t nabiman .

docker-run:
	@test -n "$$NABIMAN_PASSWORD" || (echo "Set NABIMAN_PASSWORD before docker-run" >&2; exit 1)
	docker run -d --name nabiman -p 8080:8080 \
		-v nabiman-data:/var/lib/nabiman \
		-e NABIMAN_PASSWORD \
		nabiman

# Same layout as install.sh (docs/release/README.md), without the signature check
# or the versioned directories: it installs what you just built. Do not mix it with
# nabiman-update on one host.
install: build
	@echo "Installing NabiMan..."
	sudo install -d -m 700 /var/lib/nabiman
	@if [ ! -e /var/lib/nabiman/users.json ] && [ ! -e /var/lib/nabiman/password ]; then \
		test $${#NABIMAN_PASSWORD} -ge 12 || { echo "Set a 12+ character NABIMAN_PASSWORD before make install" >&2; exit 1; }; \
		sudo install -m 600 /dev/null /var/lib/nabiman/password; \
		printf '%s' "$$NABIMAN_PASSWORD" | sudo tee /var/lib/nabiman/password >/dev/null; \
	fi
	sudo install -m 755 backend/target/release/nabiman-server backend/target/release/nabiman-privileged-helper \
		backend/target/release/nabiman-recovery scripts/management-recovery.py /usr/local/bin/
	sudo rm -rf /usr/local/share/nabiman/static.new && sudo mkdir -p /usr/local/share/nabiman
	sudo cp -r frontend/build /usr/local/share/nabiman/static.new
	sudo rm -rf /usr/local/share/nabiman/static && sudo mv /usr/local/share/nabiman/static.new /usr/local/share/nabiman/static
	sudo mkdir -p /etc/nabiman
	sudo install -m 644 nabiman.service /etc/systemd/system/nabiman.service
	sudo systemctl daemon-reload
	sudo systemctl enable nabiman
	@echo "Done. Start with: sudo systemctl start nabiman"
