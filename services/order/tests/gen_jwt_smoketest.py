import os
import sys
import logging
from tests.standalone_env_setup import devtools

ACCESS_TOKEN_PATH = "/app/log/jwt-access-ordermgt.txt"
APP_CODE = 4  # from ecommerce_common.models.enums.base.AppCodeOptions


def main():
    api_host = os.environ["API_HOST"]
    api_port = os.environ["API_PORT"]
    mock_app_user_id = os.environ["APP_USER_ID"]
    issuer_url = f"http://{api_host}:{api_port}/login"
    devtools.gen_auth_token_to_file(
        filepath=ACCESS_TOKEN_PATH,
        valid_minutes=240,
        audiences=["web", "order"],
        issuer=issuer_url,
        perm_codes=[
            (APP_CODE, "can_create_return_req"),
            (APP_CODE, "can_create_product_policy"),
        ],
        quota=[
            (APP_CODE, 1, 3),  # max limit AppAuthQuotaMatCode::NumPhones
            (APP_CODE, 2, 3),  # max limit AppAuthQuotaMatCode::NumEmails
            (APP_CODE, 3, 10),  # max limit AppAuthQuotaMatCode::NumOrderLines
            (APP_CODE, 4, 11),  # max limit AppAuthQuotaMatCode::NumProductPolicies
        ],
        usr_id=int(mock_app_user_id),
    )
    logging.info("JWT access token generated successfully for smoke test")
    sys.exit(0)


if __name__ == "__main__":
    main()
