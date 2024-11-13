import pytest_asyncio
from blacksheep.testing import TestClient

from product.entry.web import app


@pytest_asyncio.fixture(scope="session", loop_scope="session")
async def mock_client() -> TestClient:
    await app.start()
    return TestClient(app)
