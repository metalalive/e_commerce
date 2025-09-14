import os
import sys
import logging
from urllib import request, error
import json
from typing import Tuple, Dict, Any, Optional
from datetime import datetime, timezone, timedelta

# Import app from ecommerce_common.util.celery
try:
    from celery.exceptions import TimeoutError as CeleryTimeoutError
    from ecommerce_common.util import _get_amqp_url
    from ecommerce_common.util.celery import app as celery_app
    from ecommerce_common.util.messaging.constants import RPC_EXCHANGE_DEFAULT_NAME

    # Import RPCproxy and RpcReplyEvent for the new test case
    from ecommerce_common.util.messaging.rpc import RPCproxy, RpcReplyEvent
except ImportError as e:
    logging.error(f"Required library or module not found: {e}")
    sys.exit(1)


API_HOST = os.environ["API_HOST"]
API_PORT = int(os.environ["API_PORT"])
MOCK_APP_USER_ID = int(os.environ["APP_USER_ID"])
VALID_PRODUCT_ID = int(os.environ["VALID_PRODUCT_ID"])
VALID_STORE_ID = int(os.environ["VALID_STORE_ID"])
ACCESS_TOKEN_PATH = "/app/log/jwt-access-ordermgt.txt"
CART_SEQ_NUM = 1  # User-defined sequence number for the cart
APP_SYSTEM_BASEPATH = os.environ["SYS_BASE_PATH"]


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

    def patch(self, endpoint: str, data, context_name: str = "API") -> Tuple[int, str]:
        url = f"{self.base_url}{endpoint}"
        json_data = json.dumps(data).encode("utf-8")
        req = request.Request(url, data=json_data, headers=self.common_headers, method="PATCH")
        return self._send_request(req, context_name=context_name)

    def get(self, endpoint: str, context_name: str = "API") -> Tuple[int, str]:
        url = f"{self.base_url}{endpoint}"
        # For GET, data is usually in URL parameters, not body.
        # The request.Request for GET should not have a 'data' argument.
        req = request.Request(url, headers=self.common_headers, method="GET")
        return self._send_request(req, context_name=context_name)


class CeleryRpcTestClient:
    def __init__(self, mq_broker_url: str) -> None:
        self.celery_app = celery_app
        self.celery_app.conf.broker_url = mq_broker_url
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


def _poll_rpc_reply_event(reply_event: RpcReplyEvent, timeout_sec: int, context_name: str) -> bool:
    """
    Helper function to poll the status of an RPC reply event until it finishes
    or a timeout occurs.
    """
    start_time = datetime.now(timezone.utc)
    while (
        not reply_event.finished
        and float((datetime.now(timezone.utc) - start_time).total_seconds()) < timeout_sec
    ):
        err = reply_event.refresh(timeout=0.3)

    logging.debug(f"Check RPC-reply-event detail body for {context_name}: {reply_event.resp_body}")
    if err and not reply_event.finished:
        logging.error(f"Error during RPC reply refresh for {context_name}: {err}")
        return False
    if not reply_event.finished:
        logging.error(f"RPC {context_name} timed out after {timeout_sec} seconds.")
        return False
    return True


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


def verify_add_to_cart_api(client: OrderApiTestClient, cart_seq_num: int) -> bool:
    logging.info(f"Verifying Add to Cart API for cart sequence number: {cart_seq_num}")
    try:
        endpoint = f"/1.2.0/cart/{cart_seq_num}"
        request_data = {
            "title": "My Test Cart",
            "lines": [
                {
                    "seller_id": VALID_STORE_ID,
                    "product_id": VALID_PRODUCT_ID,
                    "quantity": 1,
                    "applied_attr": [],
                }
            ],
        }

        # The modify_lines handler in Rust uses PATCH, so we must use client.patch()
        status_code, response_body = client.patch(
            endpoint, request_data, context_name="Add to Cart API"
        )

        if status_code == 200:
            if response_body == "{}":
                logging.info(f"Add to Cart API verification successful for cart {cart_seq_num}.")
                return True
            else:
                logging.error(f"Add to Cart API returned unexpected body: {response_body}")
                return False
        else:
            logging.error(
                f"Add to Cart API failed with status code: {status_code}, response: {response_body}"
            )
            return False

    except Exception as e:
        logging.error(f"An error occurred during Add to Cart API verification: {e}")
        return False


def verify_retrieve_cart_api(client: OrderApiTestClient, cart_seq_num: int) -> bool:
    logging.info(f"Verifying Retrieve Cart API for cart sequence number: {cart_seq_num}")
    try:
        endpoint = f"/1.2.0/cart/{cart_seq_num}"

        status_code, response_body = client.get(endpoint, context_name="Retrieve Cart API")

        if status_code == 200:
            try:
                response_json = json.loads(response_body)
                # Expected structure based on CartDto
                if (
                    "title" in response_json
                    and "lines" in response_json
                    and isinstance(response_json["lines"], list)
                ):
                    if not response_json["lines"]:
                        logging.warning(
                            f"Retrieve Cart API returned an empty cart for {cart_seq_num}."
                        )
                        return False  # or True, depending on if empty cart is considered success for smoke test

                    # Check if the previously added item is present
                    found_expected_item = False
                    for line in response_json["lines"]:
                        if (
                            line.get("seller_id") == VALID_STORE_ID
                            and line.get("product_id") == VALID_PRODUCT_ID
                            and line.get("quantity") == 1
                        ):
                            found_expected_item = True
                            break
                    if found_expected_item:
                        logging.info(
                            f"Retrieve Cart API verification successful for cart {cart_seq_num}."
                        )
                        return True
                    else:
                        logging.error(
                            f"Retrieve Cart API did not find expected item in cart {cart_seq_num}."
                        )
                        return False
                else:
                    logging.error(
                        f"Retrieve Cart API returned unexpected JSON structure: {response_body}"
                    )
                    return False
            except json.JSONDecodeError:
                logging.error(f"Retrieve Cart API returned non-JSON body: {response_body}")
                return False
        else:
            logging.error(
                f"Retrieve Cart API failed with status code: {status_code}, response: {response_body}"
            )
            return False
    except Exception as e:
        logging.error(f"An error occurred during Retrieve Cart API verification: {e}")
        return False


def verify_product_price_rpc(rpc_client: CeleryRpcTestClient) -> bool:
    logging.info("Verifying Product Price RPC (creation)")
    routing_key = "rpc.order.update_store_products"

    now_utc = datetime.now(timezone.utc)
    start_after_str = now_utc.isoformat(timespec="seconds")
    end_before_str = (now_utc + timedelta(days=30)).isoformat(timespec="seconds")
    last_update_str = now_utc.isoformat(timespec="seconds")

    product_price_dto = {
        "s_id": VALID_STORE_ID,
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


def verify_order_creation_api(client: OrderApiTestClient) -> Tuple[bool, Optional[str]]:
    logging.info("Verifying Order Creation API")
    try:
        endpoint = "/1.2.0/order"
        request_data = {
            "order_lines": [
                {
                    "seller_id": VALID_STORE_ID,
                    "product_id": VALID_PRODUCT_ID,
                    "quantity": 9,
                    "applied_attr": [],
                }
            ],
            "currency": "TWD",
            "billing": {
                "contact": {
                    "first_name": "John",
                    "last_name": "Doe",
                    "emails": ["john.doe@example.com"],
                    "phones": [{"nation": 1, "number": "234567890"}],
                },
            },
            "shipping": {
                "contact": {
                    "first_name": "John",
                    "last_name": "Doe",
                    "emails": ["john.doe@example.com"],
                    "phones": [{"nation": 1, "number": "234567890"}],
                },
                "address": {
                    "country": "US",
                    "region": "State",
                    "city": "Anytown",
                    "distinct": "Downtown",  # Assuming a district/sub-city component
                    "street_name": None,
                    "detail": "123 Main St",
                },
                "option": [  # Add the missing 'option' field as a list
                    {
                        "seller_id": VALID_STORE_ID,
                        "method": "UPS",  # Example shipping method
                    }
                ],
            },
        }

        status_code, response_body = client.post(
            endpoint, request_data, context_name="Order Creation API"
        )

        if status_code == 201:
            try:
                response_json = json.loads(response_body)
                order_id = response_json.get("order_id")
                if isinstance(order_id, str) and order_id:
                    logging.info(
                        f"Order Creation API verification successful. Order ID: {order_id}"
                    )
                    return True, order_id
                else:
                    logging.error(
                        f"Order Creation API returned invalid or missing order_id: {response_body}"
                    )
                    return False, None
            except json.JSONDecodeError:
                logging.error(f"Order Creation API returned non-JSON body: {response_body}")
                return False, None
        else:
            logging.error(
                f"Order Creation API failed with status code: {status_code}, response: {response_body}"
            )
            return False, None

    except Exception as e:
        logging.error(f"An error occurred during Order Creation API verification: {e}")
        return False, None


def verify_product_stock_level_rpc(rpc_proxy_client: RPCproxy) -> bool:
    logging.info("Verifying Product Stock Level RPC (edit)")
    timeout_sec = 15.0
    try:
        # Define the request body as a list of InventoryEditStockLevelDto
        now_utc = datetime.now(timezone.utc)
        # The date-time string MUST NOT contain character 'Z' at the end, otherwise
        # Rust crate `chrono` will report deserialization error in order management
        # application.
        expiry_str = (now_utc + timedelta(days=365)).isoformat(timespec="seconds")

        req_msg_body = [
            {
                "qty_add": 15,
                "store_id": VALID_STORE_ID,
                "product_id": VALID_PRODUCT_ID,
                "expiry": expiry_str,
            }
        ]

        # Call the remote method `stock_level_edit`.
        # This will construct the routing key `rpc.order.stock_level_edit`.
        # The req_msg_body is passed as kwargs, consistent with common Celery RPC patterns.
        reply_event = rpc_proxy_client.stock_level_edit(custom_payload=req_msg_body)
        reply_event.send(body={"status": reply_event.status_opt.STARTED})

        if not _poll_rpc_reply_event(reply_event, timeout_sec, "Product Stock Level RPC"):
            return False

        result = reply_event.result

        if result["status"] == RpcReplyEvent.status_opt.SUCCESS:
            logging.info("Product Stock Level RPC verification successful.")
            return True
        elif result["status"] == RpcReplyEvent.status_opt.REMOTE_ERROR:
            logging.error(
                f"Product Stock Level RPC failed with remote error: {result.get('error', result.get('result'))}"
            )
            return False
        else:
            logging.error(
                f"Product Stock Level RPC failed with status: {result['status']}, result: {result.get('result')}, timeout: {result.get('timeout')}"
            )
            return False

    except Exception as e:
        logging.error(
            f"An unexpected error occurred during Product Stock Level RPC verification: {e}"
        )
        raise
        return False


def verify_order_payment_replica_rpc(rpc_proxy_client: RPCproxy, order_id: str) -> bool:
    logging.info(f"Verifying Order Payment Replica RPC for order ID: {order_id}")
    timeout_sec = 15.0
    try:
        req_msg_body = {"order_id": order_id}

        # Call the remote method `order_reserved_replica_payment`.
        # This will construct the routing key `rpc.order.order_reserved_replica_payment`.
        reply_event = rpc_proxy_client.order_reserved_replica_payment(custom_payload=req_msg_body)
        reply_event.send(body={"status": reply_event.status_opt.STARTED})

        if not _poll_rpc_reply_event(
            reply_event, timeout_sec, f"Order Payment Replica RPC for {order_id}"
        ):
            return False

        result = reply_event.result

        if result["status"] == RpcReplyEvent.status_opt.SUCCESS:
            response_data = result.get("result")
            if response_data and isinstance(response_data, dict):
                # Examine the response data, check for order_id and lines
                if response_data.get("oid") == order_id and "lines" in response_data:
                    # Additional checks for usr_id, currency, and billing
                    if all(key in response_data for key in ["usr_id", "currency", "billing"]):
                        if response_data["usr_id"] == MOCK_APP_USER_ID:
                            logging.info(
                                f"Order Payment Replica RPC verification successful for order ID: {order_id}. Received {len(response_data['lines'])} order lines."
                            )
                            return True
                        else:
                            logging.error(
                                f"Order Payment Replica RPC returned mismatching usr_id for order ID: {order_id}. Expected {MOCK_APP_USER_ID}, got {response_data['usr_id']}."
                            )
                            return False
                    else:
                        logging.error(
                            f"Order Payment Replica RPC response missing required fields (usr_id, currency, billing) for order ID: {order_id}, response: {response_data}"
                        )
                        return False
                else:
                    logging.error(
                        f"Order Payment Replica RPC returned unexpected data structure or mismatch for order ID: {order_id}, response: {response_data}"
                    )
                    return False
            else:
                logging.error(
                    f"Order Payment Replica RPC returned empty or invalid result for order ID: {order_id}, result: {response_data}"
                )
                return False
        elif result["status"] == RpcReplyEvent.status_opt.REMOTE_ERROR:
            logging.error(
                f"Order Payment Replica RPC failed with remote error: {result.get('error', result.get('result'))}"
            )
            return False
        else:
            logging.error(
                f"Order Payment Replica RPC failed with status: {result['status']}, result: {result.get('result')}, timeout: {result.get('timeout')}"
            )
            return False

    except Exception as e:
        logging.error(
            f"An unexpected error occurred during Order Payment Replica RPC verification for order ID {order_id}: {e}"
        )
        raise
        return False


def verify_order_inventory_replica_rpc(rpc_proxy_client: RPCproxy) -> bool:
    logging.info("Verifying Order Inventory Replica RPC")
    timeout_sec = 16.0
    try:
        now_utc = datetime.now(timezone.utc)
        # Set a reasonable time window to cover potential order creation time
        start_time_str = (now_utc - timedelta(minutes=5)).isoformat(timespec="seconds")
        end_time_str = (now_utc + timedelta(minutes=5)).isoformat(timespec="seconds")

        req_msg_body = {
            "start": start_time_str,
            "end": end_time_str,
        }

        # The RPCproxy method name corresponds to the routing key suffix
        reply_event = rpc_proxy_client.order_reserved_replica_inventory(custom_payload=req_msg_body)
        reply_event.send(body={"status": reply_event.status_opt.STARTED})

        if not _poll_rpc_reply_event(reply_event, timeout_sec, "Order Inventory Replica RPC"):
            return False

        result = reply_event.result

        if result["status"] == RpcReplyEvent.status_opt.SUCCESS:
            response_data = result.get("result")
            if response_data and isinstance(response_data, dict):
                reservations = response_data.get("reservations")
                returns = response_data.get("returns")

                if isinstance(reservations, list) and isinstance(returns, list):
                    logging.info(
                        f"Order Inventory Replica RPC verification successful. Found {len(reservations)} reservations and {len(returns)} returns."
                    )
                    # For a smoke test, we primarily check for successful RPC call and valid structure.
                    # Deeper validation of content would depend on specific test cases.
                    return True
                else:
                    logging.error(
                        f"Order Inventory Replica RPC returned invalid lists for reservations/returns (expected lists): {response_data}"
                    )
                    return False
            else:
                logging.error(
                    f"Order Inventory Replica RPC returned empty or invalid result (expected dict): {response_data}"
                )
                return False
        elif result["status"] == RpcReplyEvent.status_opt.REMOTE_ERROR:
            logging.error(
                f"Order Inventory Replica RPC failed with remote error: {result.get('error', result.get('result'))}"
            )
            return False
        else:
            logging.error(
                f"Order Inventory Replica RPC failed with status: {result['status']}, result: {result.get('result')}, timeout: {result.get('timeout')}"
            )
            return False

    except Exception as e:
        logging.error(
            f"An unexpected error occurred during Order Inventory Replica RPC verification: {e}"
        )
        raise
        return False


def verify_order_payment_status_update_rpc(rpc_proxy_client: RPCproxy, order_id: str) -> bool:
    logging.info(f"Verifying Order Payment Status Update RPC for order ID: {order_id}")
    timeout_sec = 16.0
    try:
        now_utc = datetime.now(timezone.utc)
        charge_time_str = now_utc.isoformat(timespec="seconds")

        req_msg_body = {
            "oid": order_id,
            "charge_time": charge_time_str,
            "lines": [
                {
                    "seller_id": VALID_STORE_ID,
                    "product_id": VALID_PRODUCT_ID,
                    "attr_set_seq": 0,  # Assuming a default for smoke test
                    "qty": 5,  # Quantity used in order creation
                }
            ],
        }

        reply_event = rpc_proxy_client.order_reserved_update_payment(custom_payload=req_msg_body)
        reply_event.send(body={"status": reply_event.status_opt.STARTED})

        if not _poll_rpc_reply_event(
            reply_event, timeout_sec, f"Order Payment Status Update RPC for {order_id}"
        ):
            return False

        result = reply_event.result

        if result["status"] == RpcReplyEvent.status_opt.SUCCESS:
            response_data = result.get("result")
            if response_data and isinstance(response_data, dict):
                # For a successful update, the 'lines' list in the error DTO should be empty
                if response_data.get("oid") == order_id and response_data.get("lines") == []:
                    logging.info(
                        f"Order Payment Status Update RPC verification successful for order ID: {order_id}."
                    )
                    return True
                else:
                    logging.error(
                        f"Order Payment Status Update RPC returned unexpected data structure or non-empty error lines for order ID: {order_id}, response: {response_data}"
                    )
                    return False
            else:
                logging.error(
                    f"Order Payment Status Update RPC returned empty or invalid result (expected dict) for order ID: {order_id}, result: {response_data}"
                )
                return False
        elif result["status"] == RpcReplyEvent.status_opt.REMOTE_ERROR:
            logging.error(
                f"Order Payment Status Update RPC failed with remote error: {result.get('error', result.get('result'))}"
            )
            return False
        else:
            logging.error(
                f"Order Payment Status Update RPC failed with status: {result['status']}, result: {result.get('result')}, timeout: {result.get('timeout')}"
            )
            return False

    except Exception as e:
        logging.error(
            f"An unexpected error occurred during Order Payment Status Update RPC verification for order ID {order_id}: {e}"
        )
        raise
        return False


def verify_order_discard_unpaid_rpc(rpc_proxy_client: RPCproxy) -> bool:
    logging.info("Verifying Order Discard Unpaid Lines RPC")
    timeout_sec = 15.0
    try:
        # The discard_unpaid_lines RPC endpoint in Rust expects a JsnVal,
        # but doesn't strictly require a specific structure for the input.
        # An empty dictionary or None can be passed for `custom_payload`.
        # The `common_setup!` macro in Rust deserializes to JsnVal.
        req_msg_body = {}  # Empty dictionary as per observation in Rust code

        reply_event = rpc_proxy_client.order_reserved_discard_unpaid(custom_payload=req_msg_body)
        reply_event.send(body={"status": reply_event.status_opt.STARTED})

        if not _poll_rpc_reply_event(reply_event, timeout_sec, "Order Discard Unpaid Lines RPC"):
            return False

        result = reply_event.result

        if result["status"] == RpcReplyEvent.status_opt.SUCCESS:
            # For discard_unpaid_lines, a successful response typically has no 'result' content (None)
            if result.get("result") is None:
                logging.info("Order Discard Unpaid Lines RPC verification successful.")
                return True
            else:
                logging.error(
                    f"Order Discard Unpaid Lines RPC returned unexpected result content: {result.get('result')}"
                )
                return False
        elif result["status"] == RpcReplyEvent.status_opt.REMOTE_ERROR:
            logging.error(
                f"Order Discard Unpaid Lines RPC failed with remote error: {result.get('error', result.get('result'))}"
            )
            return False
        else:
            logging.error(
                f"Order Discard Unpaid Lines RPC failed with status: {result['status']}, result: {result.get('result')}, timeout: {result.get('timeout')}"
            )
            return False
    except Exception as e:
        logging.error(
            f"An unexpected error occurred during Order Discard Unpaid Lines RPC verification: {e}"
        )
        raise


def verify_order_return_api(client: OrderApiTestClient, order_id: str) -> bool:
    logging.info(f"Verifying Order Return API for order ID: {order_id}")
    try:
        endpoint = f"/1.2.0/order/{order_id}/return"
        request_data = [
            {
                "seller_id": VALID_STORE_ID,
                "product_id": VALID_PRODUCT_ID,
                "attr_set_seq": 0,  # Assuming default for smoke test, adjust if actual order has specific attributes
                "quantity": 2,
            }
        ]

        status_code, response_body = client.patch(
            endpoint, request_data, context_name="Order Return API"
        )

        if status_code == 200:
            if response_body == "{}":
                logging.info(f"Order Return API verification successful for order ID: {order_id}.")
                return True
            else:
                logging.error(f"Order Return API returned unexpected body: {response_body}")
                return False
        else:
            logging.error(
                f"Order Return API failed with status code: {status_code}, response: {response_body}"
            )
            return False

    except Exception as e:
        logging.error(f"An error occurred during Order Return API verification: {e}")
        return False


def verify_order_refund_replica_rpc(rpc_proxy_client: RPCproxy, order_id: str) -> bool:
    logging.info(f"Verifying Order Refund Replica RPC for order ID: {order_id}")
    timeout_sec = 17.0
    try:
        now_utc = datetime.now(timezone.utc)
        # Set a reasonable time window to cover the time of the return request
        # The return request would have been processed very recently by the API
        start_time_str = (now_utc - timedelta(minutes=1)).isoformat(timespec="seconds")
        end_time_str = (now_utc + timedelta(minutes=1)).isoformat(timespec="seconds")

        req_msg_body = {"start": start_time_str, "end": end_time_str}

        reply_event = rpc_proxy_client.order_returned_replica_refund(custom_payload=req_msg_body)
        reply_event.send(body={"status": reply_event.status_opt.STARTED})

        if not _poll_rpc_reply_event(
            reply_event, timeout_sec, f"Order Refund Replica RPC for {order_id}"
        ):
            return False

        result = reply_event.result

        if result["status"] == RpcReplyEvent.status_opt.SUCCESS:
            response_data = result.get("result")
            if response_data and isinstance(response_data, dict) and order_id in response_data:
                if isinstance(response_data[order_id], list) and len(response_data[order_id]) > 0:
                    logging.info(
                        f"Order Refund Replica RPC verification successful for order ID: {order_id}. Found {len(response_data[order_id])} refund lines."
                    )
                    return True
                else:
                    logging.error(
                        f"Order Refund Replica RPC returned an empty or invalid list of refund lines for order ID: {order_id}, response: {response_data}"
                    )
                    return False
            else:
                logging.error(
                    f"Order Refund Replica RPC returned empty or invalid result (expected dict with order ID key) for order ID: {order_id}, result: {response_data}"
                )
                return False
        elif result["status"] == RpcReplyEvent.status_opt.REMOTE_ERROR:
            logging.error(
                f"Order Refund Replica RPC failed with remote error: {result.get('error', result.get('result'))}"
            )
            return False
        else:
            logging.error(
                f"Order Refund Replica RPC failed with status: {result['status']}, result: {result.get('result')}, timeout: {result.get('timeout')}"
            )
            return False

    except Exception as e:
        logging.error(
            f"An unexpected error occurred during Order Refund Replica RPC verification for order ID {order_id}: {e}"
        )
        raise


def refresh_currency_exchange_rpc(rpc_client: CeleryRpcTestClient) -> bool:
    logging.info("Refreshing Currency Exchange Rate RPC")
    routing_key = "rpc.order.currency_exrate_refresh"
    # The currency_refresh RPC endpoint does not require any specific message body.
    # Therefore, args and kwargs can be empty.
    success, response_detail = rpc_client.send_rpc_task(
        routing_key,
        args=[],
        kwargs={},
        context_name="Currency Exchange Rate Refresh RPC",
    )
    if success:
        logging.info("Currency Exchange Rate Refresh RPC verification successful.")
    else:
        logging.error(f"Currency Exchange Rate Refresh RPC failed: {response_detail}")
    return success


def main() -> None:
    exit_code = 1
    logging.info("Smoke test started")
    secrets_path = os.path.join(APP_SYSTEM_BASEPATH, "common/data/secrets.json")
    MSG_Q_BROKER_URL = _get_amqp_url(secrets_path=secrets_path)

    try:
        api_client = OrderApiTestClient(API_HOST, API_PORT, ACCESS_TOKEN_PATH)
        rpc_client = CeleryRpcTestClient(MSG_Q_BROKER_URL)

        # Initialize RPCproxy once in main
        # The ttl_secs for rpc.order.stock_level_edit is 180, for rpc.order.order_reserved_replica_payment is 60.
        # Use the maximum of these or a reasonable default for RPCproxy itself.
        # If specific timeouts are needed per RPC call, they can be set on the reply_event.
        # For simplicity, let's use a common timeout for the proxy that is sufficient for all RPC calls using it.
        # The most frequent timeout is 60s for order replica payment.
        rpc_proxy_common_timeout = 60  # seconds
        rpc_proxy_client = RPCproxy(
            dst_app_name="order",
            src_app_name="smoke_test_client",
            srv_basepath=APP_SYSTEM_BASEPATH,
            reply_timeout_sec=rpc_proxy_common_timeout,
        )

        all_tests_passed = True
        created_order_id = None  # Initialize to None

        if not verify_product_policy_api(api_client):
            all_tests_passed = False

        if not verify_product_price_rpc(rpc_client):
            all_tests_passed = False

        # Pass the created rpc_proxy_client instance
        if not verify_product_stock_level_rpc(rpc_proxy_client):
            all_tests_passed = False

        if not verify_add_to_cart_api(api_client, CART_SEQ_NUM):
            all_tests_passed = False

        if not verify_retrieve_cart_api(api_client, CART_SEQ_NUM):
            all_tests_passed = False

        # Refresh currency exchange rate before order creation
        if not refresh_currency_exchange_rpc(rpc_client):
            all_tests_passed = False

        # Verify order creation via the web API
        order_creation_success, order_id = verify_order_creation_api(api_client)
        if not order_creation_success:
            all_tests_passed = False
        else:
            created_order_id = order_id  # Store the order ID

        if created_order_id:
            logging.info(f"Successfully created order with ID: {created_order_id}")
            # Verify order payment replica RPC after order creation
            # Pass the created rpc_proxy_client instance
            if not verify_order_payment_replica_rpc(rpc_proxy_client, created_order_id):
                all_tests_passed = False
            # Verify order inventory replica RPC after payment replica
            if not verify_order_inventory_replica_rpc(rpc_proxy_client):
                all_tests_passed = False
            # New function: Verify order payment status update RPC after inventory replica
            if not verify_order_payment_status_update_rpc(rpc_proxy_client, created_order_id):
                all_tests_passed = False
            # New function: Verify discarding of unpaid order lines
            if not verify_order_discard_unpaid_rpc(rpc_proxy_client):
                all_tests_passed = False
            # New function: Verify order return request
            if not verify_order_return_api(api_client, created_order_id):
                all_tests_passed = False
            # New function: Verify order refund replica RPC after order return
            if all_tests_passed and not verify_order_refund_replica_rpc(
                rpc_proxy_client, created_order_id
            ):
                all_tests_passed = False
            # TODO , verify following endpoints
            # - RPC with routing key, rpc.order.stock_return_cancelled
            # - web app with URI "/cart/:seq_num" , DELETE
        else:
            logging.error("No order ID was created, skipping dependent RPC tests.")
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
        raise
    finally:
        # Ensure rpc_proxy_client is deleted at the end of main
        if "rpc_proxy_client" in locals() and rpc_proxy_client is not None:
            del rpc_proxy_client

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
