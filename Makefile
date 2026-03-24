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
	cd frontend && npm test -- --watchAll=false

clean:
	rm -rf backend/target frontend/build frontend/node_modules backend/static

docker-build:
	docker build -t nabiman .

docker-run:
	docker run -d --name nabiman -p 8080:8080 \
		-v nabiman-data:/var/lib/nabiman \
		-e NABIMAN_PASSWORD=changeme \
		nabiman

install: build
	@echo "Installing NabiMan..."
	sudo cp backend/target/release/nabiman-server /usr/local/bin/
	sudo mkdir -p /var/lib/nabiman
	sudo cp nabiman.service /etc/systemd/system/
	sudo systemctl daemon-reload
	sudo systemctl enable nabiman
	@echo "Done. Start with: sudo systemctl start nabiman"
