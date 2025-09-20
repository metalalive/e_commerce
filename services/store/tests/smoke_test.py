import os
import sys
import json
import logging
from datetime import datetime, timedelta
import urllib.request
from importlib import import_module
from typing import Optional, List, Dict

from ecommerce_common.util.celery import app as celery_app
from ecommerce_common.util.messaging.constants import RPC_EXCHANGE_DEFAULT_NAME

API_HOST = os.environ["API_HOST"]
API_PORT = os.environ["API_PORT"]
MOCK_APP_USER_ID = int(os.environ["APP_USER_ID"])
VALID_PRODUCT_ID = int(os.environ["VALID_PRODUCT_ID"])
ACCESS_TOKEN_PATH = "/app/log/jwt-access-storefront.txt"
MOCK_STAFF_USER_IDS = [int(s.strip()) for s in os.environ["APP_STAFF_USER_IDS"].split(",")]


def _get_common_headers(access_token: str) -> Dict[str, str]:
    """
    Constructs and returns common HTTP headers for API requests.
    """
    web_frontend_url = "http://localhost:8006"
    return {
        "Authorization": f"Bearer {access_token}",
        "Content-Type": "application/json",
        "Origin": web_frontend_url,
    }


def _create_store_profile(headers: dict) -> Optional[List[Dict]]:
    """
    Performs a POST request to create store profiles.
    Returns the response body (list of dicts, corresponding to StoreProfileCreatedDto) on success (HTTP 201), None otherwise.
    """
    url = f"http://{API_HOST}:{API_PORT}/profiles"
    logging.info("Attempting to create store profiles.")
    logging.debug(f"actual rednerred URL : {url}")

    # Request body moved here from main()
    store_profile_request_body = [
        {
            "label": "Test Store Front",
            "supervisor_id": MOCK_APP_USER_ID,
            "currency": "USD",  # Matches StoreCurrency enum
            "active": True,
            "emails": [{"addr": "test@example.com"}],
            "phones": [{"country_code": "1", "line_number": "5551234567"}],
            "location": {
                # valid country code from common/data/nationality_code.json
                "country": ["US", "Unit State"],
                "locality": "Springfield",
                "street": "123 Main St",
                "detail": "Suite 100",
                "floor": 1,
            },
        }
    ]

    data = json.dumps(store_profile_request_body).encode("utf-8")
    try:
        req = urllib.request.Request(url, data=data, headers=headers, method="POST")
        with urllib.request.urlopen(req, timeout=5) as response:
            status = response.getcode()
            response_json = json.load(response)
            if int(status) == 201:
                logging.info(f"Store profile creation successful. Status: {status}")
                logging.debug(f"Response body: {response_json}")
                return response_json
            else:
                logging.error(
                    f"Store profile creation failed. Unexpected status: {status}, Response: {response_json}"
                )
                return None
    except urllib.error.HTTPError as e:
        status = e.code
        response_text = e.read().decode("utf-8")
        logging.error(f"Store profile creation failed. Status: {status}, Response: {response_text}")
        return None
    except urllib.error.URLError as e:
        logging.error(f"A URL error occurred during store profile creation: {e.reason}")
        return None
    except Exception as e:
        logging.error(f"An unexpected error occurred during store profile creation operation: {e}")
        return None


def _delete_store_profiles(headers: dict, store_ids: List[int]) -> bool:
    """
    Performs a DELETE request to delete store profiles.
    Returns True on success, False otherwise.
    """
    if not store_ids:
        logging.info("No store profiles to delete.")
        return True

    ids_str = ",".join(map(str, store_ids))
    url = f"http://{API_HOST}:{API_PORT}/profiles?ids={ids_str}"
    logging.debug(f"Attempting to delete store profiles with IDs: {ids_str}")
    try:
        req = urllib.request.Request(url, headers=headers, method="DELETE")
        with urllib.request.urlopen(req, timeout=5) as response:
            status = response.getcode()
            if int(status) == 204:  # No Content for successful DELETE
                logging.debug(f"Store profiles deletion successful. Status: {status}")
                return True
            else:
                response_text = response.read().decode("utf-8")
                logging.error(
                    f"Store profiles deletion failed. Unexpected status: {status}, Response: {response_text}"
                )
                return False
    except urllib.error.HTTPError as e:
        status = e.code
        response_text = e.read().decode("utf-8")
        logging.error(
            f"Store profiles deletion failed. Status: {status}, Response: {response_text}"
        )
        return False
    except urllib.error.URLError as e:
        logging.error(f"A URL error occurred during store profiles deletion: {e.reason}")
        return None
    except Exception as e:
        logging.error(f"An unexpected error occurred during store profiles deletion operation: {e}")
        return False


def _add_user_to_staff(headers: dict, store_id: int) -> bool:
    """
    Performs a PATCH request to add the MOCK_APP_USER_ID to a store's staff.
    Returns True on success, False otherwise.
    """
    url = f"http://{API_HOST}:{API_PORT}/profile/{store_id}/staff"
    logging.info(f"Attempting to add user {MOCK_STAFF_USER_IDS} to staff for store {store_id}.")

    now = datetime.now()
    # Set end_before sufficiently in the future, e.g., one year from now
    one_year_later = now + timedelta(days=365)

    staff_request_body = [
        {
            "staff_id": uid,
            "start_after": now.isoformat(),
            "end_before": one_year_later.isoformat(),
        }
        for uid in MOCK_STAFF_USER_IDS
    ]

    data = json.dumps(staff_request_body).encode("utf-8")
    try:
        req = urllib.request.Request(url, data=data, headers=headers, method="PATCH")
        with urllib.request.urlopen(req, timeout=5) as response:
            status = response.getcode()
            if int(status) == 200:  # Assuming 200 OK for successful PATCH
                logging.info(
                    f"Users {MOCK_STAFF_USER_IDS} successfully added to staff for store {store_id}. Status: {status}"
                )
                return True
            else:
                response_text = response.read().decode("utf-8")
                logging.error(
                    f"Adding users {MOCK_STAFF_USER_IDS} to staff failed. Unexpected status: {status}, Response: {response_text}"
                )
                return False
    except urllib.error.HTTPError as e:
        logging.error(
            f"Adding users {MOCK_STAFF_USER_IDS} to staff failed. Status: {e.code}, Response: {e.read().decode('utf-8')}"
        )
        return False
    except (urllib.error.URLError, Exception) as e:
        logging.error(f"An error occurred during adding user to staff operation: {e}")
        return False


def _add_product_price(headers: dict, store_id: int) -> bool:
    """
    Performs a PATCH request to add product prices to a store profile.
    Returns True on success, False otherwise.
    """
    url = f"http://{API_HOST}:{API_PORT}/profile/{store_id}/products"
    logging.info(
        f"Attempting to add product price for product {VALID_PRODUCT_ID} to store {store_id}."
    )

    now = datetime.now()
    one_year_later = now + timedelta(days=365)

    product_price_request_body = [
        {
            "product_id": VALID_PRODUCT_ID,
            "base_price": 1000,  # Example base price
            "start_after": now.isoformat(),
            "end_before": one_year_later.isoformat(),
            "attrs_charge": [],  # No additional attributes for simplicity in smoke test
        }
    ]

    data = json.dumps(product_price_request_body).encode("utf-8")
    try:
        req = urllib.request.Request(url, data=data, headers=headers, method="PATCH")
        with urllib.request.urlopen(req, timeout=5) as response:
            status = response.getcode()
            if int(status) == 200:
                logging.info(
                    f"Product price for {VALID_PRODUCT_ID} successfully added to store {store_id}. Status: {status}"
                )
                return True
            else:
                response_text = response.read().decode("utf-8")
                logging.error(
                    f"Adding product price failed. Unexpected status: {status}, Response: {response_text}"
                )
                return False
    except urllib.error.HTTPError as e:
        logging.error(
            f"Adding product price failed. Status: {e.code}, Response: {e.read().decode('utf-8')}"
        )
        return False
    except (urllib.error.URLError, Exception) as e:
        logging.error(f"An error occurred during adding product price operation: {e}")
        return False


def _get_store_profile_via_rpc(celeryapp, store_id: int) -> Optional[Dict]:
    """
    Retrieves an existing store profile via a Celery RPC call and returns the response.
    """
    logging.info(f"Attempting to retrieve store profile {store_id} via RPC.")
    try:
        task = celeryapp.send_task(
            name="store.api.rpc.get_shop_profile",
            kwargs={"req": {"store_id": store_id}},
            exchange=RPC_EXCHANGE_DEFAULT_NAME,
            routing_key="rpc.storefront.get_profile",
        )
        result = task.get(timeout=10)  # Wait for the result with a 10-second timeout
        if result and "error" not in result:
            logging.info(f"Successfully retrieved store profile {store_id} via RPC.")
            logging.debug(f"RPC response: {result}")
            return result
        else:
            logging.error(
                f"Failed to retrieve store profile {store_id} via RPC. Response: {result}"
            )
            return None
    except Exception as e:
        logging.error(f"An error occurred during RPC call to get store profile: {e}")
        return None


def main(celeryapp) -> int:
    clean_on_exit = os.environ["SMOKETEST_CLEAN_DATA"].lower() == "true"
    logging.info("Smoke test started")

    store_profile_ids = []  # List to hold created store IDs for cleanup
    headers = None
    exit_code = 1  # Assume failure until successful completion

    try:
        with open(ACCESS_TOKEN_PATH, "r") as f:
            access_token = f.read().strip()

        headers = _get_common_headers(access_token)

        # Test case: Create a new store profile
        # The request body for /profiles endpoint (add_profiles handler) is NewStoreProfilesReqBody,
        # which is expected to be a dictionary with a 'root' key holding a list of NewStoreProfileDto.
        store_profile_response = _create_store_profile(headers)
        assert store_profile_response, "Failed to create store profile."
        assert len(store_profile_response) > 0, "No store profile created in response."

        created_profile_dto = store_profile_response[0]
        store_id = created_profile_dto.get("id")
        supervisor_id = created_profile_dto.get("supervisor_id")

        assert store_id is not None, "Failed to retrieve 'id' from store profile creation response."
        assert supervisor_id == MOCK_APP_USER_ID, "Supervisor ID mismatch in created store profile."

        store_profile_ids.append(store_id)
        logging.info(f"Created store profile with ID: {store_id}, Supervisor ID: {supervisor_id}")

        # Test case: Add user to staff list
        logging.info("Attempting to add user to store staff.")
        staff_add_success = _add_user_to_staff(headers, store_id)
        assert staff_add_success, f"Failed to add users to staff for store {store_id}."

        logging.info(
            f"Users {MOCK_STAFF_USER_IDS} successfully added to staff for store {store_id}."
        )

        # Test case: Add product price
        logging.info("Attempting to add product price to store.")
        product_price_add_success = _add_product_price(headers, store_id)
        assert (
            product_price_add_success
        ), f"Failed to add product price for product {VALID_PRODUCT_ID} to store {store_id}."
        logging.info("Product price successfully added to store.")

        # Test case: Retrieve store profile via RPC after updates and verify its contents
        logging.info(
            f"Attempting to retrieve store profile {store_id} via RPC for verification after updates."
        )
        retrieved_profile = _get_store_profile_via_rpc(celeryapp, store_id)
        assert retrieved_profile, f"Failed to retrieve store profile {store_id} via RPC."
        assert (
            retrieved_profile["supervisor_id"] == MOCK_APP_USER_ID
        ), "Retrieved profile supervisor ID mismatch."
        assert retrieved_profile["label"] == "Test Store Front", "Retrieved profile label mismatch."
        assert retrieved_profile["active"] is True, "Retrieved profile active status mismatch."

        # Verify emails
        assert len(retrieved_profile["emails"]) == 1, "Expected 1 email in retrieved profile."
        assert (
            retrieved_profile["emails"][0]["addr"] == "test@example.com"
        ), "Retrieved email address mismatch."

        # Verify phones
        assert (
            len(retrieved_profile["phones"]) == 1
        ), "Expected 1 phone number in retrieved profile."
        assert (
            retrieved_profile["phones"][0]["country_code"] == "1"
        ), "Retrieved phone country code mismatch."
        assert (
            retrieved_profile["phones"][0]["line_number"] == "5551234567"
        ), "Retrieved phone line number mismatch."

        # Verify location
        assert retrieved_profile["location"]["country"] == [
            "US",
            "Unit State",
        ], "Retrieved location country mismatch."
        assert (
            retrieved_profile["location"]["locality"] == "Springfield"
        ), "Retrieved location locality mismatch."
        assert (
            retrieved_profile["location"]["street"] == "123 Main St"
        ), "Retrieved location street mismatch."
        assert (
            retrieved_profile["location"]["detail"] == "Suite 100"
        ), "Retrieved location detail mismatch."
        assert retrieved_profile["location"]["floor"] == 1, "Retrieved location floor mismatch."

        # Verify staff information (persisted)
        assert len(retrieved_profile["staff"]) == len(
            MOCK_STAFF_USER_IDS
        ), f"Expected {len(MOCK_STAFF_USER_IDS)} staff members, but got {len(retrieved_profile['staff'])}."
        retrieved_staff_ids = {s["staff_id"] for s in retrieved_profile["staff"]}
        expected_staff_ids = set(MOCK_STAFF_USER_IDS)
        assert (
            retrieved_staff_ids == expected_staff_ids
        ), f"Retrieved staff IDs {retrieved_staff_ids} do not match expected IDs {expected_staff_ids}."
        logging.info(
            f"Store profile {store_id} successfully retrieved and verified via RPC, including staff information."
        )

        logging.info("Smoke test passed.")
        exit_code = 0  # Set success code

    except FileNotFoundError:
        logging.error(f"Access token file not found at {ACCESS_TOKEN_PATH}")
    except Exception as e:
        logging.error(f"An unexpected error occurred: {e}")
    finally:
        # Conditionally perform cleanup based on --clean-on-exit argument
        if clean_on_exit:
            if store_profile_ids and headers:
                if not _delete_store_profiles(headers, store_profile_ids):
                    logging.error(
                        f"Failed to delete store profiles {store_profile_ids} during cleanup."
                    )
        else:
            logging.info("Cleanup skipped as requested by --clean-on-exit=false.")

    return exit_code


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
    # The original main() function was empty, but based on the overall structure,
    # it implies that shared_ctx and celery_app configuration might be needed
    # for other future tests or if RPC calls were to be made from this file.
    # For now, only HTTP requests are made, so direct Celery config might not be strictly needed,
    # but I'll keep the import_module and celery_app config as it was in the product smoke test.
    try:
        cfg_mod_path = os.environ["APP_SETTINGS"]
        _settings = import_module(cfg_mod_path)
        celery_app.config_from_object(_settings)
        sys.exit(main(celery_app))
    except Exception as e:
        logging.error(f"Could not load application settings or initialize RPC: {e}")
        sys.exit(66)  # cannot open input setting file
