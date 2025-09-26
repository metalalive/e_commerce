import os
import sys
import logging
from tests.standalone_env_setup import devtools

ACCESS_TOKEN_PATH = "/app/tmp/log/dev/jwt-access-storefront.txt"
APP_CODE = 5  # from ecommerce_common.models.enums.base.AppCodeOptions


def main():
    api_host = os.environ["API_HOST"]
    api_port = os.environ["API_PORT"]
    mock_app_user_id = int(os.environ["APP_USER_ID"])
    issuer_url = f"http://{api_host}:{api_port}/login"
    devtools.gen_auth_token_to_file(
        filepath=ACCESS_TOKEN_PATH,
        valid_minutes=600,
        audiences=["web", "store"],
        issuer=issuer_url,
        perm_codes=[
            (APP_CODE, "view_storeprofile"),
            (APP_CODE, "change_storeprofile"),
            (APP_CODE, "add_storeprofile"),
            (APP_CODE, "add_storeproductavailable"),
            (APP_CODE, "change_storeproductavailable"),
            (APP_CODE, "delete_storeproductavailable"),
        ],
        quota=[
            (APP_CODE, 1, 5),  # max limit QuotaMatCode.MAX_NUM_STORES is 5
            (APP_CODE, 2, 6),  # max limit QuotaMatCode.MAX_NUM_STAFF is 6
            (APP_CODE, 3, 7),  # max limit QuotaMatCode.MAX_NUM_EMAILS is 7
            (APP_CODE, 4, 8),  # max limit QuotaMatCode.MAX_NUM_PHONES is 8
            (APP_CODE, 5, 9),  # max limit QuotaMatCode.MAX_NUM_PRODUCTS is 9
        ],
        usr_id=mock_app_user_id,
    )
    logging.info("JWT access token generated successfully for smoke test")
    sys.exit(0)


if __name__ == "__main__":
    main()
