.PHONY: craft-up craft-down craft-sandbox-image craft-backend-image craft-refresh-images craft-check-images

craft-up:
	deployment/helm/dev/craft-up.sh

craft-down:
	deployment/helm/dev/craft-down.sh

craft-sandbox-image:
	docker build -t lumendotapp/sandbox:dev backend/lumen/server/features/build/sandbox/image
	kind load docker-image lumendotapp/sandbox:dev --name lumen-dev

# Rebuild the image the in-cluster sandbox-proxy/api-server run, then restart them.
craft-backend-image:
	docker build -t lumendotapp/lumen-backend:dev backend/
	kind load docker-image lumendotapp/lumen-backend:dev --name lumen-dev
	kubectl rollout restart deploy/lumen-sandbox-proxy deploy/lumen-api-server -n lumen

# Refresh everything: backend + sandbox images, PodTemplate, proxy/api restart.
craft-refresh-images:
	deployment/helm/dev/refresh-images.sh

craft-check-images:
	deployment/helm/dev/refresh-images.sh --check || true
