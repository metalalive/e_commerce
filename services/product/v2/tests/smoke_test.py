import os
import sys
import json
import time
import logging
import urllib.request
import argparse  # Import argparse
from importlib import import_module
from typing import Optional, List, Dict

from ecommerce_common.util.celery import app as celery_app
from ecommerce_common.util.messaging.constants import RPC_EXCHANGE_DEFAULT_NAME

API_HOST = os.environ["API_HOST"]
API_PORT = os.environ["API_PORT"]
MOCK_APP_USER_ID = os.environ["APP_USER_ID"]
ACCESS_TOKEN_PATH = "/app/log/jwt-access-prodmgt.txt"


# New function to parse command-line arguments
def parse_args():
    parser = argparse.ArgumentParser(description="Run product service smoke tests.")
    parser.add_argument(
        "--clean-on-exit",
        type=lambda x: x.lower() == "true",  # Converts "true" to True, others to False
        default=True,  # Default to cleaning up
        help="Whether to clean up created resources at the end of the test (true/false). Default is true.",
    )
    return parser.parse_args()


def _create_tag(headers: dict, request_body: dict) -> Optional[dict]:
    """
    Performs a POST request to create a tag.
    Returns the response body (dict) on success (HTTP 201), None otherwise.
    """
    url = f"http://{API_HOST}:{API_PORT}/tag"
    logging.info("Attempting to create tag.")
    data = json.dumps(request_body).encode("utf-8")
    try:
        req = urllib.request.Request(url, data=data, headers=headers, method="POST")
        with urllib.request.urlopen(req, timeout=5) as response:
            status = response.getcode()
            response_json = json.load(response)
            if int(status) == 201:
                logging.info(f"Tag creation successful. Status: {status}")
                logging.debug(f"Response body: {response_json}")
                return response_json["node"]
            else:
                logging.error(
                    f"Tag creation failed. Unexpected status: {status}, Response: {response_json}"
                )
                return None
    except urllib.error.HTTPError as e:
        status = e.code
        response_text = e.read().decode("utf-8")
        logging.error(f"Tag creation failed. Status: {status}, Response: {response_text}")
        return None
    except urllib.error.URLError as e:
        logging.error(f"A URL error occurred during tag creation: {e.reason}")
        return None
    except Exception as e:
        logging.error(f"An unexpected error occurred during tag creation operation: {e}")
        return None


def _create_attribute_label(headers: dict, request_body: list) -> Optional[list]:
    """
    Performs a POST request to create an attribute label.
    Returns the response body (list) on success (HTTP 200 or 201), None otherwise.
    """
    url = f"http://{API_HOST}:{API_PORT}/attributes"
    logging.info("Attempting to create attribute label.")
    data = json.dumps(request_body).encode("utf-8")
    try:
        req = urllib.request.Request(url, data=data, headers=headers, method="POST")
        with urllib.request.urlopen(req, timeout=5) as response:
            status = response.getcode()
            response_json = json.load(response)
            if int(status) in [200, 201]:
                logging.info(f"Attribute label creation successful. Status: {status}")
                logging.debug(f"Response body: {response_json}")
                return response_json
            else:
                logging.error(
                    f"Attribute label creation failed. Unexpected status: {status}, Response: {response_json}"
                )
                return None
    except urllib.error.HTTPError as e:
        status = e.code
        response_text = e.read().decode("utf-8")
        logging.error(
            f"Attribute label creation failed. Status: {status}, Response: {response_text}"
        )
        return None
    except urllib.error.URLError as e:
        logging.error(f"A URL error occurred during attribute label creation: {e.reason}")
        return None
    except Exception as e:
        logging.error(
            f"An unexpected error occurred during attribute label creation operation: {e}"
        )
        return None


def _delete_tags(headers: dict, tid: str) -> bool:
    """
    Performs a DELETE request to delete tags.
    Returns True on success, False otherwise.
    """
    if not tid:
        logging.info("No tag to delete.")
        return True

    url = f"http://{API_HOST}:{API_PORT}/tag/{tid}"
    logging.debug(f"Attempting to delete tags with IDs: {tid}")
    try:
        req = urllib.request.Request(url, headers=headers, method="DELETE")
        with urllib.request.urlopen(req, timeout=5) as response:
            status = response.getcode()
            if int(status) == 204:  # No Content for successful DELETE
                logging.debug(f"Tags deletion successful. Status: {status}")
                return True
            else:
                response_text = response.read().decode("utf-8")
                logging.error(
                    f"Tags deletion failed. Unexpected status: {status}, Response: {response_text}"
                )
                return False
    except urllib.error.HTTPError as e:
        status = e.code
        response_text = e.read().decode("utf-8")
        logging.error(f"Tags deletion failed. Status: {status}, Response: {response_text}")
        return False
    except urllib.error.URLError as e:
        logging.error(f"A URL error occurred during tags deletion: {e.reason}")
        return False
    except Exception as e:
        logging.error(f"An unexpected error occurred during tags deletion operation: {e}")
        return False


def _delete_attribute_labels(headers: dict, ids: List[str]) -> bool:
    """
    Performs a DELETE request to delete attribute labels.
    Returns True on success, False otherwise.
    """
    if not ids:
        logging.info("No attribute labels to delete.")
        return True

    url_ids = ",".join(ids)
    url = f"http://{API_HOST}:{API_PORT}/attributes?ids={url_ids}"
    logging.debug(f"Attempting to delete attribute labels with IDs: {url_ids}")
    try:
        req = urllib.request.Request(url, headers=headers, method="DELETE")
        with urllib.request.urlopen(req, timeout=5) as response:
            status = response.getcode()
            if int(status) == 204:  # No Content for successful DELETE
                logging.debug(f"Attribute labels deletion successful. Status: {status}")
                return True
            else:
                response_text = response.read().decode("utf-8")
                logging.error(
                    f"Attribute labels deletion failed. Unexpected status: {status}, Response: {response_text}"
                )
                return False
    except urllib.error.HTTPError as e:
        status = e.code
        response_text = e.read().decode("utf-8")
        logging.error(
            f"Attribute labels deletion failed. Status: {status}, Response: {response_text}"
        )
        return False
    except urllib.error.URLError as e:
        logging.error(f"A URL error occurred during attribute labels deletion: {e.reason}")
        return False
    except Exception as e:
        logging.error(
            f"An unexpected error occurred during attribute labels deletion operation: {e}"
        )
        return False


def _create_saleable_item(headers: dict, tag_id: str, attr_label_id: str) -> Optional[str]:
    """
    Performs a POST request to create a saleable item.
    Returns the ID of the created item on success (HTTP 201), None on failure.
    """
    url = f"http://{API_HOST}:{API_PORT}/item"
    logging.info("Attempting to create saleable item.")

    # Construct a meaningful request body using the IDs for the 'Electronics' tag
    # and 'Color' attribute label (mocked, as dynamic retrieval is not possible
    # without modifying existing functions).
    request_body = {
        "name": "Fancy Gadget",
        "visible": True,
        "tags": [tag_id],
        "attributes": [
            {
                "id_": attr_label_id,
                "value": "Red",  # A meaningful value for the 'Color' attribute (dtype=String)
            }
        ],
        "media_set": ["img-001-sku-A", "vid-001-sku-A"],  # Example media resource IDs
    }

    data = json.dumps(request_body).encode("utf-8")
    try:
        req = urllib.request.Request(url, data=data, headers=headers, method="POST")
        with urllib.request.urlopen(req, timeout=5) as response:
            status = response.getcode()
            if int(status) == 201:
                response_json = json.load(response)
                item_id = response_json.get("id_")
                if item_id:
                    logging.info(f"Saleable item creation successful. Status: {status}")
                    logging.debug(f"Response body: {response_json}")
                    return str(item_id)
                else:
                    logging.error(
                        f"Saleable item creation successful (status {status}) but missing 'id_' in response: {response_json}"
                    )
                    return None
            else:
                response_text = response.read().decode("utf-8")
                logging.error(
                    f"Saleable item creation failed. Unexpected status: {status}, Response: {response_text}"
                )
                return None
    except urllib.error.HTTPError as e:
        status = e.code
        response_text = e.read().decode("utf-8")
        logging.error(f"Saleable item creation failed. Status: {status}, Response: {response_text}")
        return None
    except urllib.error.URLError as e:
        logging.error(f"A URL error occurred during saleable item creation: {e.reason}")
        return None
    except Exception as e:
        logging.error(f"An unexpected error occurred during saleable item creation operation: {e}")
        return None


def _delete_saleable_item(headers: dict, item_id: str) -> bool:
    """
    Performs a DELETE request to delete a saleable item.
    Returns True on success, False otherwise.
    """
    if not item_id:
        logging.info("No saleable item to delete.")
        return True

    url = f"http://{API_HOST}:{API_PORT}/item/{item_id}"
    logging.debug(f"Attempting to delete saleable item with ID: {item_id}")
    try:
        req = urllib.request.Request(url, headers=headers, method="DELETE")
        with urllib.request.urlopen(req, timeout=5) as response:
            status = response.getcode()
            if int(status) == 204:  # No Content for successful DELETE
                logging.debug(f"Saleable item deletion successful. Status: {status}")
                return True
            else:
                response_text = response.read().decode("utf-8")
                logging.error(
                    f"Saleable item deletion failed. Unexpected status: {status}, Response: {response_text}"
                )
                return False
    except urllib.error.HTTPError as e:
        status = e.code
        response_text = e.read().decode("utf-8")
        logging.error(f"Saleable item creation failed. Status: {status}, Response: {response_text}")
        return False
    except urllib.error.URLError as e:
        logging.error(f"A URL error occurred during saleable item creation: {e.reason}")
        return False
    except Exception as e:
        logging.error(f"An unexpected error occurred during saleable item deletion operation: {e}")
        return False


def _get_saleable_item_via_celery(item_id: str, app_user_id: str) -> Optional[Dict]:
    """
    Retrieves a saleable item using the Celery RPC interface.
    Returns the item data (dict) on success, None otherwise.
    """
    logging.info(f"Attempting to retrieve saleable item {item_id} via Celery RPC.")
    try:
        task = celery_app.send_task(
            "product.api.rpc.get_product",
            args=[[int(item_id)], app_user_id],
            queue="rpc_productmgt_get_product",
            routing_key="rpc.product.get_product",
            exchange=RPC_EXCHANGE_DEFAULT_NAME,
        )
        logging.debug(f"Celery task sent with ID: {task.id}")

        result_wrapper = task.get(timeout=10)

        if result_wrapper and isinstance(result_wrapper, dict) and "result" in result_wrapper:
            item_list = result_wrapper["result"]
            if item_list and len(item_list) > 0:
                item_data = item_list[0]
                logging.info(f"Successfully retrieved item {item_id} via Celery RPC.")
                logging.debug(f"Celery RPC response: {item_data}")
                return item_data
            else:
                logging.warning(f"Celery RPC returned an empty list for item {item_id}.")
                return None
        else:
            logging.error(
                f"Failed to retrieve item {item_id} via Celery RPC. Unexpected response format: {result_wrapper}"
            )
            return None
    except Exception as e:
        logging.error(f"An error occurred during Celery RPC call to retrieve item {item_id}: {e}")
        return None


def main():
    args = parse_args()  # Parse command-line arguments
    web_frontend_url = "http://localhost:8006"
    logging.info("Smoke test started")

    tag_id = None
    saleable_item_id = None
    attr_label_id = None
    headers = None
    exit_code = 1  # Assume failure until successful completion

    try:
        # Test case: Create a new tag
        with open(ACCESS_TOKEN_PATH, "r") as f:
            access_token = f.read().strip()

        headers = {
            "Authorization": f"Bearer {access_token}",
            "Content-Type": "application/json",
            "Origin": web_frontend_url,
        }

        # Test case: Create a new tag
        tag_request_body = {"name": "Electronics", "parent": None}
        tag_response = _create_tag(headers, tag_request_body)
        assert tag_response, "Failed to create 'Electronics' tag."
        tag_id = tag_response.get("id_")
        assert tag_id, "Failed to retrieve ID from 'Electronics' tag creation response."
        logging.info(f"Created 'Electronics' tag with ID: {tag_id}")

        # Test case: Create a new attribute label
        attribute_request_body = [{"name": "Color", "dtype": 3}]
        attr_label_response = _create_attribute_label(headers, attribute_request_body)
        assert (
            attr_label_response and len(attr_label_response) > 0
        ), "Failed to create 'Color' attribute label."
        attr_label_id = attr_label_response[0].get("id_")
        assert (
            attr_label_id
        ), "Failed to retrieve ID from 'Color' attribute label creation response."
        logging.info(f"Created 'Color' attribute label with ID: {attr_label_id}")

        # New Test case: Create a saleable item
        saleable_item_id = _create_saleable_item(headers, tag_id, attr_label_id)
        assert saleable_item_id, "Failed to create saleable item."
        logging.info(f"Created saleable item with ID: {saleable_item_id}")

        time.sleep(3)
        retrieved_item = _get_saleable_item_via_celery(saleable_item_id, int(MOCK_APP_USER_ID))
        assert (
            retrieved_item
        ), f"Failed to retrieve saleable item {saleable_item_id} via Celery RPC."
        assert retrieved_item.get("id_") == int(
            saleable_item_id
        ), f"Retrieved item ID ({retrieved_item.get('id_')}) does not match expected ({saleable_item_id})."
        assert "attributes" in retrieved_item, "Retrieved item missing 'attributes' field."
        assert len(retrieved_item["attributes"]) > 0, "Retrieved item has no attributes."
        retrieved_attr = retrieved_item["attributes"][0]
        assert (
            retrieved_attr["label"].get("id_") == attr_label_id
        ), f"Retrieved item attribute ID ({retrieved_attr.get('id_')}) does not match expected ({attr_label_id})."
        assert (
            retrieved_attr.get("value") == "Red"
        ), f"Retrieved item attribute value ({retrieved_attr.get('value')}) does not match expected ('Red')."
        logging.info("Saleable item successfully retrieved via Celery RPC and validated.")

        logging.info("Smoke test passed")
        exit_code = 0  # Set success code

    except FileNotFoundError:
        logging.error(f"Access token file not found at {ACCESS_TOKEN_PATH}")
    except Exception as e:
        logging.error(f"An unexpected error occurred: {e}")
    finally:
        # Conditionally perform cleanup based on --clean-on-exit argument
        if args.clean_on_exit:
            # Ensure saleable item is deleted if it was created and headers are available
            if saleable_item_id and headers:
                if not _delete_saleable_item(headers, saleable_item_id):
                    logging.error(
                        f"Failed to delete saleable item {saleable_item_id} during cleanup."
                    )
            # Ensure attribute label is deleted if it was created and headers are available
            if attr_label_id and headers:
                if not _delete_attribute_labels(headers, [attr_label_id]):
                    logging.error(
                        f"Failed to delete attribute label {attr_label_id} during cleanup."
                    )
            # Ensure tag is deleted if it was created and headers are available
            if tag_id and headers:
                if not _delete_tags(headers, tag_id):
                    logging.error(f"Failed to delete tag {tag_id} during cleanup.")
        else:
            logging.info("Cleanup skipped as requested by --clean-on-exit=false.")

    sys.exit(exit_code)  # Final exit status


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
    cfg_mod_path = os.environ["APP_SETTINGS"]
    _settings = import_module(cfg_mod_path)
    celery_app.config_from_object(_settings)
    _settings.init_rpc(celery_app)
    main()
