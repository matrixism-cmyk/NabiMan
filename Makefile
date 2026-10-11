.PHONY: build build-frontend build-backend deploy test clean docker-build docker-run install

build: build-frontend build-backend deploy

build-frontend:
	cd frontend && npm ci --no-audit && npm run build

build-backend:
	cd backend && cargo build --release

deploy:
	mkdir -p backend/static
	rm -rf backend/static/*
	cp -r frontend/build/* backend/static/

test:
	cd backend && cargo test
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

install: build
	@echo "Installing NabiMan..."
	sudo mkdir -p /var/lib/nabiman
	@if [ ! -e /var/lib/nabiman/users.json ] && [ ! -e /var/lib/nabiman/password ]; then \
		test $${#NABIMAN_PASSWORD} -ge 12 || { echo "Set a 12+ character NABIMAN_PASSWORD before make install" >&2; exit 1; }; \
		sudo install -m 600 /dev/null /var/lib/nabiman/password; \
		printf '%s' "$$NABIMAN_PASSWORD" | sudo tee /var/lib/nabiman/password >/dev/null; \
	fi
	sudo cp backend/target/release/nabiman-server /usr/local/bin/
	sudo cp nabiman.service /etc/systemd/system/
	sudo systemctl daemon-reload
	sudo systemctl enable nabiman
	@echo "Done. Start with: sudo systemctl start nabiman"
