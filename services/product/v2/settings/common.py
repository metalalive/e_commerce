ROUTERS = ["product.api.web.router"]
SHARED_CONTEXT = "product.shared.SharedContext"

REPO_PKG_BASE = "product.adapter.repository"

DATABASES = {
    "tag": {"classpath": REPO_PKG_BASE + ".elasticsearch.ElasticSearchTagRepo"}
}
