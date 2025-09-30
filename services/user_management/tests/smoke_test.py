import os
import sys
import secrets
import string
import logging
import json
from datetime import datetime, UTC, timedelta
from typing import Dict, Any, List, Tuple
from urllib import request
from urllib.error import URLError
from unittest.mock import Mock
import http.cookies

# Read connection details and API endpoint from environment variables
API_HOST = os.environ["API_HOST"]
API_PORT = os.environ["API_PORT"]
DB_HOST = os.environ["DB_HOST"]
DB_PORT = os.environ["DB_PORT"]

# --- Django Setup ---
# This setup is required to use Django's ORM outside of a manage.py context.
# It assumes this script is run from the 'services/user_management/infra' directory.
sys.path.insert(0, os.path.abspath("./src"))
sys.path.insert(0, os.path.abspath("."))
os.environ.setdefault("DJANGO_SETTINGS_MODULE", "settings.development")
import django  # noqa : E402

try:
    django.setup()
except ImportError as e:
    sys.stderr.write(
        "Error: Django setup failed. Ensure DJANGO_SETTINGS_MODULE is set correctly "
        "and the 'src' directory is in the Python path.\n"
        f"Details: {e}\n"
    )
    sys.exit(1)

root_logger = logging.getLogger()
if root_logger.handlers:
    for handler in root_logger.handlers:
        root_logger.removeHandler(handler)

logging.basicConfig(
    level=logging.DEBUG,
    format="%(asctime)s - %(levelname)s - %(message)s",
    stream=sys.stdout,
)

# the django modules should be running after `django.setup()`
from django.conf import settings as django_settings  # noqa : E402
from django.middleware.csrf import get_token as csrf_get_token  # noqa : E402
from django.contrib.auth.models import Permission  # noqa : E402
from django.contrib.contenttypes.models import ContentType  # noqa : E402

from ecommerce_common.cors.middleware import conf as cors_conf  # noqa : E402
from ecommerce_common.models.constants import ROLE_ID_STAFF  # noqa : E402
from ecommerce_common.util import get_header_name  # noqa : E402
from user_management.models.base import (  # noqa : E402
    GenericUserProfile,
    GenericUserGroup,
    GenericUserAppliedRole,
    UserQuotaRelation,
    QuotaMaterial,
)
from user_management.models.auth import (  # noqa : E402
    LoginAccount,
    Role,
    UnauthResetAccountRequest,
)
from user_management.models.common import _atomicity_fn, DB_ALIAS_APPLIED  # noqa : E402

# --- Helper Functions ---


def generate_password(length=16):
    """Generates a cryptographically secure random password."""
    alphabet = string.ascii_letters + string.digits + string.punctuation
    return "".join(secrets.choice(alphabet) for _ in range(length))


def _get_request_headers():
    """Generates standard request headers for API calls, including CSRF token."""
    mock_req = Mock()
    mock_req.META = {}
    csrf_token = csrf_get_token(mock_req)
    csrf_header = get_header_name(django_settings.CSRF_HEADER_NAME)
    headers = {
        "Content-Type": "application/json",
        "Origin": cors_conf.ALLOWED_ORIGIN["web"],
        # Simulate a cross-origin request, this origin header can be any value in
        # `ALLOWED_ORIGIN` in `common/data/cors.json`
        csrf_header: csrf_token,
        "Cookie": f"{django_settings.CSRF_COOKIE_NAME}={csrf_token}",
    }
    return headers


def get_permissions_for_models(model_classes, using=None):
    """Fetches Django Permission objects for a list of model classes."""
    content_types = ContentType.objects.db_manager(using).get_for_models(*model_classes).values()
    return Permission.objects.using(using).filter(content_type__in=content_types)


def get_permissions_by_codename(codenames: list, using=None):
    """Fetches Django Permission objects by their codenames."""
    return Permission.objects.using(using).filter(codename__in=codenames)


def create_test_user(
    username: str, password: str, db_alias: str, t_now: datetime
) -> GenericUserProfile:
    """Creates a temporary user with necessary roles and permissions for the smoke test."""
    with _atomicity_fn():
        logging.info(f"Creating temporary user: '{username}'")

        # Create the user profile
        user_profile = GenericUserProfile.objects.db_manager(db_alias).create(
            first_name="Amber", last_name="Bruce"
        )

        # Create the login account with staff privileges
        LoginAccount.objects.db_manager(db_alias).create_user(
            profile=user_profile,
            username=username,
            password=password,
            is_active=True,
            password_last_updated=t_now,
        )

        # Create a role and assign necessary permissions for management tasks
        permissions = get_permissions_for_models(
            [
                Role,
                UserQuotaRelation,
                GenericUserGroup,
                GenericUserProfile,
                UnauthResetAccountRequest,
                LoginAccount,
            ],
            using=db_alias,
        )
        test_role, _ = Role.objects.using(db_alias).get_or_create(name="Smoke Test Manager Role")
        test_role.permissions.set(permissions)
        staff_role = Role.objects.using(db_alias).get(pk=ROLE_ID_STAFF)
        # Apply the roles below to the new user profile
        GenericUserAppliedRole.objects.using(db_alias).create(
            user_ref=user_profile,
            role=test_role,
            approved_by=user_profile,  # Self-approved for test purposes
        )
        GenericUserAppliedRole.objects.using(db_alias).create(
            user_ref=user_profile,
            role=staff_role,
            approved_by=user_profile,  # Self-approved for test purposes
        )
        # Update the account's staff/superuser status based on roles
        user_profile.update_account_privilege()

        logging.info(f"User '{username}' and permissions created successfully.")
        return user_profile


def setup_user_login_account(db_alias: str, username: str, t_now: datetime, usr_prof_kept: Dict):
    with _atomicity_fn():
        user_profile = GenericUserProfile.objects.get(pk=usr_prof_kept["id"])
        LoginAccount.objects.db_manager(db_alias).create_user(
            profile=user_profile,
            username=username,
            password=generate_password(),
            is_active=True,
            password_last_updated=t_now,
        )
    logging.info(
        "User profile ID : %s , kept for smoke tests in other applications" % usr_prof_kept["id"]
    )


def delete_all_test_objs(
    user_profile: GenericUserProfile,
    username: str,
    db_alias: str,
    created_profiles: List[Dict[str, Any]] = None,
):
    """Deletes the temporary user and associated role created for the smoke test."""
    logging.info(f"Cleaning up: Deleting user '{username}'...")
    try:
        if created_profiles:
            profile_ids_to_delete = [p["id"] for p in created_profiles]
            logging.info(f"Deleting test profiles with IDs: {profile_ids_to_delete}")
            GenericUserProfile.objects.using(db_alias).filter(id__in=profile_ids_to_delete).delete(
                hard=True
            )
        # Use a hard delete to permanently remove records
        # The related LoginAccount is deleted via CASCADE
        user_profile.delete(hard=True)
        # groups will be used in test cases of other applications
        # GenericUserGroup.objects.using(db_alias).filter(
        #     name__in=["Smoke Test Group A", "Smoke Test Group B"]
        # ).delete(hard=True)
        Role.objects.using(db_alias).filter(
            name__in=[
                "Smoke Test Manager Role",
                "Smoke Test Role A (Updated)",
                "Smoke Test Role B",
            ]
        ).delete()
        logging.info(f"Cleanup successful for user '{username}'.")
    except Exception as e:
        logging.error(
            f"CRITICAL: Failed to clean up test user '{username}'. Manual cleanup required. Error: {e}"
        )
        sys.exit(1)


def assign_roles_to_user(
    user_profile: GenericUserProfile, roles: List[Dict[str, Any]], db_alias: str
):
    """Assigns a list of roles (from API response) to a given user profile."""
    logging.info(f"Assigning {len(roles)} roles to user '{user_profile.account.username}'")
    role_ids = [r["id"] for r in roles]
    role_objects = Role.objects.using(db_alias).filter(id__in=role_ids)
    with _atomicity_fn():
        for role in role_objects:
            GenericUserAppliedRole.objects.using(db_alias).create(
                user_ref=user_profile,
                role=role,
                approved_by=user_profile,
            )
        user_profile.update_account_privilege()
    logging.info("Successfully assigned roles to the test user.")


# --- Main Test Logic ---


def test_manage_roles(auth_info: Dict[str, Any], db_alias: str) -> list:
    """
    Tests creating, updating, and deleting roles via the API.
    """
    logging.info("Testing role management (Create, Update, Delete)...")
    api_endpoint = "/roles"
    base_url = f"http://{API_HOST}:{API_PORT}{api_endpoint}"
    access_token = auth_info["access_token"]

    headers = _get_request_headers()
    headers["Authorization"] = f"Bearer {access_token}"

    # --- 1. Get some permissions to assign to roles ---
    perm_codenames = [
        "add_saleableitem",
        "view_saleableitem",
        "change_saleableitem",
        "add_saleablepackage",
        "view_saleablepackage",
        "delete_saleablepackage",
    ]
    permissions = get_permissions_by_codename(perm_codenames, using=db_alias)
    perm_ids = list(permissions.values_list("pk", flat=True))
    if len(perm_ids) < len(perm_codenames):
        raise AssertionError("Not enough permissions found to conduct role management test.")

    # --- 2. Create roles (POST) ---
    logging.info("Attempting to create roles...")
    create_payload = [
        {"name": "Smoke Test Role A", "permissions": perm_ids[0:2]},
        {"name": "Smoke Test Role B", "permissions": perm_ids[2:4]},
        {"name": "Delete Me Role", "permissions": perm_ids[4:6]},
    ]
    data = json.dumps(create_payload).encode("utf-8")
    req = request.Request(base_url, data=data, headers=headers, method="POST")

    with request.urlopen(req) as response:
        if response.status != 201:
            raise AssertionError(f"Role creation failed with status {response.status}")
        created_roles = json.loads(response.read())

    assert len(created_roles) == 3, "Expected 3 roles to be created."
    logging.info(f"Successfully created {len(created_roles)} roles.")

    # --- 3. Update a role (PUT) ---
    logging.info("Attempting to update a role...")
    role_to_update = created_roles[0]
    update_payload = [
        {
            "id": role_to_update["id"],
            "name": "Smoke Test Role A (Updated)",
            "permissions": role_to_update["permissions"] + [perm_ids[2]],
        }
    ]
    data = json.dumps(update_payload).encode("utf-8")
    req = request.Request(base_url, data=data, headers=headers, method="PUT")

    with request.urlopen(req) as response:
        if response.status != 200:
            raise AssertionError(f"Role update failed with status {response.status}")

    logging.info("Successfully updated a role.")

    # --- 4. Delete a role (DELETE) ---
    logging.info("Attempting to delete a role...")
    role_to_delete = next(r for r in created_roles if r["name"] == "Delete Me Role")
    delete_id = role_to_delete["id"]
    delete_url = f"{base_url}?ids={delete_id}"
    req = request.Request(delete_url, headers=headers, method="DELETE")

    with request.urlopen(req) as response:
        if response.status != 204:
            raise AssertionError(f"Role deletion failed with status {response.status}")

    logging.info(f"Successfully deleted role ID: {delete_id}.")

    # --- 5. Return the remaining roles for subsequent tests ---
    remaining_roles = [r for r in created_roles if r["id"] != delete_id]
    # The first role was updated, so let's reflect that in what we return.
    updated_role_info = update_payload[0]
    for i, role in enumerate(remaining_roles):
        if role["id"] == updated_role_info["id"]:
            remaining_roles[i] = updated_role_info
            break

    assert len(remaining_roles) >= 2, "Expected at least 2 roles to remain."
    logging.info("Role management test passed.")
    return remaining_roles


def test_manage_usrgrps(auth_info: Dict[str, Any]) -> list:
    """
    Tests creating user groups via the API.
    """
    logging.info("Testing user group management (Create)...")
    api_endpoint = "/groups"
    base_url = f"http://{API_HOST}:{API_PORT}{api_endpoint}"
    access_token = auth_info["access_token"]

    headers = _get_request_headers()
    headers["Authorization"] = f"Bearer {access_token}"

    # --- 1. Create user groups (POST) ---
    logging.info("Attempting to create user groups...")
    create_payload = [
        {
            "name": "Smoke Test Group A",
            "roles": [],
            "quota": [],
            "emails": [],
            "phones": [],
            "locations": [],
        },
        {
            "name": "Smoke Test Group B",
            "new_parent": 0,
            "roles": [],
            "quota": [],
            "emails": [],
            "phones": [],
            "locations": [],
        },
    ]
    data = json.dumps(create_payload).encode("utf-8")
    req = request.Request(base_url, data=data, headers=headers, method="POST")

    with request.urlopen(req) as response:
        if response.status != 201:
            raise AssertionError(f"User group creation failed with status {response.status}")
        created_groups = json.loads(response.read())

    assert len(created_groups) == 2, "Expected 2 user groups to be created."
    logging.info(f"Successfully created {len(created_groups)} user groups.")
    # --- 2. Return created groups ---
    logging.info("User group management test passed.")
    return created_groups


def test_create_usrprofs(
    auth_info: Dict[str, Any], roles: list, groups: list, db_alias: str
) -> list:
    """
    Tests creating user profiles via the API.
    """
    logging.info("Testing user profile management (Create)...")
    api_endpoint = "/profiles"
    base_url = f"http://{API_HOST}:{API_PORT}{api_endpoint}"
    headers = _get_request_headers()
    headers["Authorization"] = f"Bearer {auth_info['access_token']}"

    # --- 1. Get quota materials for assignment ---
    # Pick some quota from the payment app for testing
    quota_materials = QuotaMaterial.objects.using(db_alias).filter(app_code=7)
    if not quota_materials.exists():
        raise AssertionError("No quota materials found for testing.")
    quota_material_id = quota_materials.first().pk
    # Pick some quota from this user-management app for testing
    quota_materials = QuotaMaterial.objects.using(db_alias).filter(app_code=1)
    usr_email_quota_mat_id = quota_materials.get(
        mat_code=QuotaMaterial._MatCodeOptions.MAX_NUM_EMAILS
    ).pk
    usr_phone_quota_mat_id = quota_materials.get(
        mat_code=QuotaMaterial._MatCodeOptions.MAX_NUM_PHONE_NUMBERS
    ).pk
    quota_materials = QuotaMaterial.objects.using(db_alias).filter(app_code=5)
    store_quota_mat_id = quota_materials.get(mat_code=1).pk
    store_staff_quota_mat_id = quota_materials.get(mat_code=2).pk
    store_emails_quota_mat_id = quota_materials.get(mat_code=3).pk
    store_phones_quota_mat_id = quota_materials.get(mat_code=4).pk
    store_products_quota_mat_id = quota_materials.get(mat_code=5).pk
    # --- 2. Create user profiles (POST) ---
    logging.info("Attempting to create user profiles...")
    role_ids = [r["id"] for r in roles]
    grp_ids = [g["id"] for g in groups]
    expiry_time = (datetime.now(UTC) + timedelta(days=365)).isoformat()
    create_payload = [
        {
            "first_name": "John",
            "last_name": "Smoke",
            "groups": [{"group": grp_ids[0]}],
            "phones": [
                {"line_number": "0912093383", "country_code": "887"},
                {"line_number": "0912345678", "country_code": "886"},
            ],
            "locations": [],
            "emails": [{"addr": "john.smoke@example.com"}],
            "roles": [{"role": role_ids[0], "expiry": expiry_time}],
            "quota": [
                {"material": usr_email_quota_mat_id, "maxnum": 2, "expiry": expiry_time},
                {"material": usr_phone_quota_mat_id, "maxnum": 2, "expiry": expiry_time},
                {"material": quota_material_id, "maxnum": 10, "expiry": expiry_time},
            ],
        },
        {
            "first_name": "Jane",
            "last_name": "Tester",
            "groups": [{"group": grp_ids[1]}],
            "phones": [],
            "locations": [],
            "emails": [{"addr": "jane.tester@example.com"}],
            "roles": [{"role": role_ids[1], "expiry": expiry_time}],
            "quota": [
                {"material": usr_email_quota_mat_id, "maxnum": 2, "expiry": expiry_time},
                {"material": usr_phone_quota_mat_id, "maxnum": 2, "expiry": expiry_time},
                {"material": quota_material_id, "maxnum": 5, "expiry": expiry_time},
            ],
        },
        {
            "first_name": "Charlie",
            "last_name": "Delta",
            "groups": [{"group": grp_ids[0]}],
            "phones": [],
            "locations": [],
            "emails": [{"addr": "charlie.delta@example.com"}],
            "roles": [{"role": role_ids[0], "expiry": expiry_time}],
            "quota": [
                {"material": usr_email_quota_mat_id, "maxnum": 1, "expiry": expiry_time},
                {"material": usr_phone_quota_mat_id, "maxnum": 1, "expiry": expiry_time},
                {"material": quota_material_id, "maxnum": 15, "expiry": expiry_time},
            ],
        },
        {
            "first_name": "Esspreso",
            "last_name": "Fatima",
            "groups": [{"group": grp_ids[0]}],
            "phones": [
                {"line_number": "0730281190", "country_code": "92"},
            ],
            "locations": [],
            "emails": [{"addr": "esspro.fatimah@example.com"}],
            "roles": [{"role": role_ids[0], "expiry": expiry_time}],
            "quota": [
                {"material": usr_email_quota_mat_id, "maxnum": 2, "expiry": expiry_time},
                {"material": usr_phone_quota_mat_id, "maxnum": 2, "expiry": expiry_time},
                {"material": quota_material_id, "maxnum": 16, "expiry": expiry_time},
                {"material": store_quota_mat_id, "maxnum": 5, "expiry": expiry_time},
                {"material": store_staff_quota_mat_id, "maxnum": 6, "expiry": expiry_time},
                {"material": store_emails_quota_mat_id, "maxnum": 7, "expiry": expiry_time},
                {"material": store_phones_quota_mat_id, "maxnum": 8, "expiry": expiry_time},
                {"material": store_products_quota_mat_id, "maxnum": 9, "expiry": expiry_time},
            ],
        },
        {
            "first_name": "Geoffery",
            "last_name": "Halluciation",
            "groups": [{"group": grp_ids[1]}],
            "phones": [
                {"line_number": "0281190101", "country_code": "163"},
            ],
            "locations": [],
            "emails": [{"addr": "geff.hallus@example.com"}],
            "roles": [{"role": role_ids[0], "expiry": expiry_time}],
            "quota": [
                {"material": usr_email_quota_mat_id, "maxnum": 1, "expiry": expiry_time},
                {"material": usr_phone_quota_mat_id, "maxnum": 1, "expiry": expiry_time},
                {"material": quota_material_id, "maxnum": 8, "expiry": expiry_time},
                {"material": store_products_quota_mat_id, "maxnum": 15, "expiry": expiry_time},
            ],
        },
        {
            "first_name": "Ioweed",
            "last_name": "Jotsckegikaegh",
            "groups": [{"group": grp_ids[1]}],
            "phones": [
                {"line_number": "0281190555", "country_code": "163"},
            ],
            "locations": [],
            "emails": [{"addr": "iowe.jgika@example.com"}],
            "roles": [{"role": role_ids[0], "expiry": expiry_time}],
            "quota": [
                {"material": usr_email_quota_mat_id, "maxnum": 1, "expiry": expiry_time},
                {"material": usr_phone_quota_mat_id, "maxnum": 1, "expiry": expiry_time},
                {"material": quota_material_id, "maxnum": 8, "expiry": expiry_time},
                {"material": store_products_quota_mat_id, "maxnum": 20, "expiry": expiry_time},
            ],
        },
    ]
    data = json.dumps(create_payload).encode("utf-8")
    req = request.Request(base_url, data=data, headers=headers, method="POST")

    with request.urlopen(req) as response:
        if response.status != 201:
            raise AssertionError(f"User profile creation failed with status {response.status}")
        created_profiles = json.loads(response.read())

    assert len(created_profiles) == 6, "Expected 6 user profiles to be created."
    logging.info(f"Successfully created {len(created_profiles)} user profiles.")
    return created_profiles


def test_update_usrprof(auth_info: Dict[str, Any], profile_to_update: dict):
    """
    Tests updating a user profile via the API.
    """
    logging.info("Testing user profile management (Update)...")
    api_endpoint = "/profiles"
    base_url = f"http://{API_HOST}:{API_PORT}{api_endpoint}"
    headers = _get_request_headers()
    headers["Authorization"] = f"Bearer {auth_info['access_token']}"

    logging.info("Attempting to update a user profile...")
    # For a PUT request, we typically send the complete object representation.
    # We'll use the data from the creation response, and modify it.
    profile_to_update["last_name"] = "Smoke-Updated"
    profile_to_update["quota"][0]["maxnum"] = 20  # Update quota limit
    update_payload = [profile_to_update]

    data = json.dumps(update_payload).encode("utf-8")
    req = request.Request(base_url, data=data, headers=headers, method="PUT")

    with request.urlopen(req) as response:
        if response.status != 200:
            raise AssertionError(f"User profile update failed with status {response.status}")

    logging.info("Successfully updated a user profile.")


def test_query_usrprof(auth_info: Dict[str, Any], profile_id: int):
    """
    Tests retrieving and validating a user profile via the API.
    """
    logging.info("Testing user profile management (Query and Validate)...")
    api_endpoint = "/profile"
    base_url = f"http://{API_HOST}:{API_PORT}{api_endpoint}"
    headers = _get_request_headers()
    headers["Authorization"] = f"Bearer {auth_info['access_token']}"

    logging.info("Validating updated profile...")
    get_url = f"{base_url}/{profile_id}"
    req = request.Request(get_url, headers=headers, method="GET")

    with request.urlopen(req) as response:
        if response.status != 200:
            raise AssertionError(
                f"Failed to retrieve updated profile with status {response.status}"
            )
        retrieved_profile = json.loads(response.read())

    assert retrieved_profile["last_name"] == "Smoke-Updated", "Last name was not updated correctly."
    assert retrieved_profile["quota"][0]["maxnum"] == 20, "Quota was not updated correctly."
    logging.info("Successfully validated updated profile.")


def test_account_activation(
    auth_info: Dict[str, Any], profile_to_activate: dict
) -> Tuple[int, int]:
    """
    Tests requesting account activation for a user profile.
    """
    logging.info("Testing account activation request...")
    api_endpoint = "/account/activate"
    base_url = f"http://{API_HOST}:{API_PORT}{api_endpoint}"
    headers = _get_request_headers()
    headers["Authorization"] = f"Bearer {auth_info['access_token']}"

    if not profile_to_activate.get("emails"):
        raise AssertionError("Profile for activation must have an email address.")

    # Use the first email for activation request
    email_id = profile_to_activate["emails"][0]["id"]
    profile_id = profile_to_activate["id"]
    logging.info(f"Requesting activation for profile ID {profile_id} via email ID {email_id}")

    payload = [{"profile": profile_id, "email": email_id}]
    data = json.dumps(payload).encode("utf-8")
    req = request.Request(base_url, data=data, headers=headers, method="POST")

    with request.urlopen(req) as response:
        if response.status != 201:
            body = response.read().decode("utf-8")
            raise AssertionError(
                f"Account activation request failed with status {response.status}, " f"body: {body}"
            )

    logging.info("Account activation request sent successfully.")
    return (profile_id, email_id)


def test_api_login(username: str, password: str) -> str:
    """
    Attempts to log in via API, validates the response, and returns the
    JWT refresh token value from the Set-Cookie header.
    """
    api_endpoint = "/login"
    login_url = f"http://{API_HOST}:{API_PORT}{api_endpoint}"
    logging.info(f"Attempting to log in via API at: {login_url}")

    headers = _get_request_headers()
    payload = {"username": username, "password": password}

    data = json.dumps(payload).encode("utf-8")
    req = request.Request(login_url, data=data, headers=headers, method="POST")

    with request.urlopen(req) as response:
        json.loads(response.read())
        set_cookie_header = response.getheader("Set-Cookie")

    cookie_name = django_settings.JWT_NAME_REFRESH_TOKEN
    # The refresh token is returned in an HttpOnly cookie.
    if (
        set_cookie_header
        and set_cookie_header.strip().startswith(f"{cookie_name}=")
        and "httponly" in set_cookie_header.lower()
    ):
        cookie = http.cookies.SimpleCookie()
        cookie.load(set_cookie_header)
        refresh_token = cookie.get(cookie_name).value
        logging.info("API Login successful. Refresh token received in cookie.")
        # logging.debug(f"refresh_token = {refresh_token}")
        return refresh_token
    else:
        raise ValueError("Refresh token not found in login response's Set-Cookie header.")


def test_auth_token_refreshing(refresh_token: str) -> Dict[str, Any]:
    """
    Tests the access token refresh mechanism using a valid refresh token.
    """
    api_endpoint_refresh = "/refresh_access_token"
    refresh_token_url = f"http://{API_HOST}:{API_PORT}{api_endpoint_refresh}"
    logging.info(f"Attempting to refresh access token at: {refresh_token_url}")

    headers = _get_request_headers()
    refresh_cookie_name = django_settings.JWT_NAME_REFRESH_TOKEN
    headers["Cookie"] += f"; {refresh_cookie_name}={refresh_token}"

    audience = "user_management"
    url_with_query = f"{refresh_token_url}?audience={audience}"
    req = request.Request(url_with_query, headers=headers, method="GET")

    with request.urlopen(req) as response:
        response_data = json.loads(response.read())

    if "access_token" in response_data and "jwks_url" in response_data:
        logging.info("Access token refreshed successfully.")
        logging.debug(f"access_token: {response_data['access_token'][:30]}...")
        logging.debug(f"jwks_url: {response_data['jwks_url']}")
        return response_data
    else:
        raise ValueError("Access token or JWKS URL not found in refresh response.")


def test_public_jwks(token_refresh_response: Dict[str, Any]):
    """
    Tests the JWKS endpoint to ensure it returns a valid set of public keys.
    """
    jwks_url = token_refresh_response.get("jwks_url")
    if not jwks_url:
        raise ValueError("jwks_url not found in token refresh response")

    logging.info(f"Attempting to fetch JWKS from: {jwks_url}")

    req = request.Request(jwks_url, method="GET")

    with request.urlopen(req) as response:
        response_data = json.loads(response.read())

    if "keys" not in response_data or not isinstance(response_data["keys"], list):
        raise ValueError("Invalid JWKS format: 'keys' field is missing or not a list")

    if not response_data["keys"]:
        raise ValueError("JWKS 'keys' array is empty")

    for key in response_data["keys"]:
        if "kid" not in key:
            raise ValueError(f"A key in JWKS is missing 'kid' field. Key: {key}")

    logging.info("JWKS endpoint returned a valid key set.")


def main():
    """Main function to run the smoke test."""
    username = "AmberBruce"
    password = generate_password()
    user_profile = None
    created_profiles_for_cleanup = []
    db_alias = DB_ALIAS_APPLIED  # hard-coded database alias for test
    t_now = datetime.now(UTC)

    logging.info("Starting smoke test...")

    try:
        # 1. Create a temporary user for the test
        user_profile = create_test_user(username, password, db_alias, t_now)

        # 2. Run the API login test
        refresh_token = test_api_login(username, password)

        # 3. Request a new access token
        auth_info = test_auth_token_refreshing(refresh_token)

        # 4. Fetch and validate public keys
        test_public_jwks(auth_info)

        # 5. Test role management (Create/Update/Delete)
        roles = test_manage_roles(auth_info, db_alias)

        assign_roles_to_user(user_profile, roles, db_alias)

        # 6. Test user group management (Create)
        groups = test_manage_usrgrps(auth_info)

        # 7. Test user profile management (Create, Update, Query)
        created_profiles_for_cleanup = test_create_usrprofs(auth_info, roles, groups, db_alias)
        setup_user_login_account(db_alias, "CicadaDino", t_now, created_profiles_for_cleanup.pop(3))
        setup_user_login_account(db_alias, "EelFrog", t_now, created_profiles_for_cleanup.pop(3))
        setup_user_login_account(
            db_alias, "GopherHeron", t_now, created_profiles_for_cleanup.pop(3)
        )

        profile_to_update = created_profiles_for_cleanup[0]
        test_update_usrprof(auth_info, profile_to_update)
        test_query_usrprof(auth_info, profile_to_update["id"])

        profile_to_activate = created_profiles_for_cleanup[1]
        test_account_activation(auth_info, profile_to_activate)

        # TODO
        # test email notification with following cases , each works with specific API endpoint.
        # - account activation request / confirmation on new user
        # - password-reset request / confirmation on new user
        # - account username recovery for new user
        # test deactivate login account of an existing user

    except (URLError, ValueError, AssertionError) as e:
        logging.error(f"SMOKE TEST FAILED: {e}")
        sys.exit(1)

    finally:
        # 8. Clean up: Delete the temporary user regardless of the result
        if user_profile:
            delete_all_test_objs(user_profile, username, db_alias, created_profiles_for_cleanup)

    logging.info("SMOKE TEST PASSED")
    sys.exit(0)


### export DB_HOST=127.0.0.1
### export API_HOST=127.0.0.1
### export API_PORT=8000
### python3 infra/smoke_test.py
if __name__ == "__main__":
    main()
