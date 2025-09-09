import os
import sys
import logging
from urllib import request, error
import json
from typing import Tuple, Dict, Any
from datetime import datetime, timezone, timedelta

# Import app from ecommerce_common.util.celery
try:
    from ecommerce_common.util.celery import app as celery_app
    from celery.exceptions import TimeoutError as CeleryTimeoutError
    from ecommerce_common.util.messaging.constants import RPC_EXCHANGE_DEFAULT_NAME
except ImportError as e:
    logging.error(f"Required library or module not found: {e}")
    sys.exit(1)


API_HOST = os.environ["API_HOST"]
API_PORT = int(os.environ["API_PORT"])
MOCK_APP_USER_ID = int(os.environ["APP_USER_ID"])
VALID_PRODUCT_ID = int(os.environ["VALID_PRODUCT_ID"])
ACCESS_TOKEN_PATH = "/app/log/jwt-access-ordermgt.txt"
# Assuming CELERY_BROKER_URL is set in the environment and used by ecommerce_common.util.celery.app
CELERY_BROKER_URL = os.environ["CELERY_BROKER_URL"]


class OrderApiTestClient:
    def __init__(self, api_host: str, api_port: int, access_token_path: str) -> None:
        self.base_url = f"http://{api_host}:{api_port}"
        self.access_token = self._load_access_token(access_token_path)
        self.common_headers = {
            "Content-Type": "application/json",
            "Authorization": f"Bearer {self.access_token}",
        }

    def _load_access_token(self, path: str) -> str:
        try:
            with open(path, "r") as f:
                token = f.read().strip()
                if not token:
                    raise ValueError("Access token not found or empty.")
                return token
        except FileNotFoundError:
            logging.error(f"Access token file not found at {path}")
            raise
        except Exception as e:
            logging.error(f"An unexpected error occurred during access token loading: {e}")
            raise

    def _send_request(self, req: request.Request, context_name: str = "API") -> Tuple[int, str]:
        try:
            with request.urlopen(req) as response:
                status_code = response.getcode()
                response_body = response.read().decode("utf-8")
                return status_code, response_body
        except error.HTTPError as e:
            logging.error(
                f"HTTP Error for {context_name}: {e.code} - {e.reason}, Response: {e.read().decode('utf-8')}"
            )
            raise
        except error.URLError as e:
            logging.error(f"URL Error for {context_name}: {e.reason}")
            raise
        except Exception as e:
            logging.error(f"An unexpected error occurred during {context_name} verification: {e}")
            raise

    def post(self, endpoint: str, data, context_name: str = "API") -> Tuple[int, str]:
        url = f"{self.base_url}{endpoint}"
        json_data = json.dumps(data).encode("utf-8")
        req = request.Request(url, data=json_data, headers=self.common_headers, method="POST")
        return self._send_request(req, context_name=context_name)


class CeleryRpcTestClient:
    def __init__(self) -> None:
        self.celery_app = celery_app
        self.celery_app.conf.broker_url = CELERY_BROKER_URL
        self.celery_app.conf.result_backend = "rpc://"
        self.default_timeout = 10  # seconds

    def send_rpc_task(
        self,
        routing_key: str,
        args: Tuple[Any, ...],
        kwargs: Dict[str, Any],
        context_name: str = "RPC",
    ) -> Tuple[bool, str]:
        try:
            task = self.celery_app.send_task(
                name="order.rpc.handler",  # Dummy task name, not used in order management rpc consumer
                args=args,
                kwargs=kwargs,
                routing_key=routing_key,
                exchange=RPC_EXCHANGE_DEFAULT_NAME,
            )
            logging.info(f"Sent RPC task with ID: {task.id}, routing key: {routing_key}")

            result_raw = task.get(timeout=self.default_timeout)

            if result_raw:
                try:
                    error_data = json.loads(result_raw)
                    logging.error(f"RPC {context_name} failed: {error_data}")
                    return False, str(error_data)
                except json.JSONDecodeError:
                    logging.error(
                        f"RPC {context_name} returned unexpected non-JSON data: {result_raw}"
                    )
                    return False, str(result_raw)
            else:
                logging.info(f"RPC {context_name} verification successful (empty response).")
                return True, ""
        except CeleryTimeoutError:
            logging.error(f"RPC {context_name} timed out after {self.default_timeout} seconds.")
            return False, "Timeout"
        except Exception as e:
            logging.error(
                f"An unexpected error occurred during RPC {context_name} verification: {e}"
            )
            return False, str(e)


def verify_product_policy_api(client: OrderApiTestClient) -> bool:
    logging.info("Verifying Product Policy API")
    try:
        endpoint = "/1.2.0/policy/products"
        request_data = [
            {
                "product_id": VALID_PRODUCT_ID,
                "auto_cancel_secs": 4200,
                "warranty_hours": 720,
                "max_num_rsv": 10,
                "min_num_rsv": 2,
            }
        ]

        status_code, response_body = client.post(
            endpoint, request_data, context_name="Product Policy API"
        )

        if status_code == 200:
            if response_body == "{}":
                logging.info("Product Policy API verification successful.")
                return True
            else:
                logging.error(f"Product Policy API returned unexpected body: {response_body}")
                return False
        else:
            logging.error(
                f"Product Policy API failed with status code: {status_code}, response: {response_body}"
            )
            return False

    except Exception:
        return False


def verify_product_price_rpc(rpc_client: CeleryRpcTestClient) -> bool:
    logging.info("Verifying Product Price RPC (creation)")
    routing_key = "rpc.order.update_store_products"

    now_utc = datetime.now(timezone.utc)
    start_after_str = now_utc.isoformat(timespec="seconds")
    end_before_str = (now_utc + timedelta(days=30)).isoformat(timespec="seconds")
    last_update_str = now_utc.isoformat(timespec="seconds")

    product_price_dto = {
        "s_id": MOCK_APP_USER_ID,
        "rm_all": False,
        "currency": "TWD",
        "deleting": {"items": None},
        "updating": [],
        "creating": [
            {
                "price": 1000,
                "start_after": start_after_str,
                "end_before": end_before_str,
                "product_id": VALID_PRODUCT_ID,
                "attributes": {
                    "extra_charge": [
                        {
                            "label_id": "color",
                            "value": "red",
                            "price": 50,
                        }
                    ],
                    "last_update": last_update_str,
                },
            }
        ],
    }

    success, response_detail = rpc_client.send_rpc_task(
        routing_key,
        args=[],  # Corresponds to `pargs` in Rust's `py_celery_deserialize_req`
        kwargs=product_price_dto,  # Corresponds to `kwargs` in Rust's `py_celery_deserialize_req`
        context_name="Product Price RPC Create",
    )
    if success:
        logging.info("Product Price RPC verification successful.")
    else:
        logging.error(f"Product Price RPC failed: {response_detail}")
    return success


def main() -> None:
    exit_code = 1
    logging.info("Smoke test started")

    try:
        api_client = OrderApiTestClient(API_HOST, API_PORT, ACCESS_TOKEN_PATH)
        rpc_client = CeleryRpcTestClient()

        all_tests_passed = True

        if not verify_product_policy_api(api_client):
            all_tests_passed = False

        if not verify_product_price_rpc(rpc_client):
            all_tests_passed = False

        if all_tests_passed:
            logging.info("Smoke test passed")
            exit_code = 0
        else:
            logging.error("Smoke test failed")

    except FileNotFoundError:
        logging.error("Smoke test failed due to missing access token file.")
    except ValueError:
        logging.error("Smoke test failed due to invalid access token.")
    except Exception as e:
        logging.error(f"Smoke test failed during setup or execution: {e}")
        logging.error("Smoke test failed.")

    sys.exit(exit_code)


if __name__ == "__main__":
    root_logger = logging.getLogger()
    if root_logger.handlers:
        for handler in root_logger.handlers:
            root_logger.removeHandler(handler)

    logging.basicConfig(
        level=logging.DEBUG,
        format="%(asctime)s - %(levelname)s - %(message)s",
        stream=sys.stdout,
    )
    main()
