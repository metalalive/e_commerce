import os
import sys
import logging
from tests.standalone_env_setup import devtools

ACCESS_TOKEN_PATH = "/app/tmp/log/dev/jwt-access-prodmgt.txt"
APP_CODE = 2  # from ecommerce_common.models.enums.base.AppCodeOptions


def main():
    api_host = os.environ["API_HOST"]
    api_port = os.environ["API_PORT"]
    mock_app_user_id = os.environ["APP_USER_ID"]
    issuer_url = f"http://{api_host}:{api_port}/login"
    devtools.gen_auth_token_to_file(
        filepath=ACCESS_TOKEN_PATH,
        valid_minutes=600,
        audiences=["web", "product"],
        issuer=issuer_url,
        perm_codes=[
            (APP_CODE, "add_producttag"),
            (APP_CODE, "delete_producttag"),
            (APP_CODE, "add_saleableitem"),
            (APP_CODE, "delete_saleableitem"),
            (APP_CODE, "add_productattributetype"),
            (APP_CODE, "delete_productattributetype"),
        ],
        quota=[
            (APP_CODE, 2, 5),  # max limit QuotaMaterialCode.NumSaleItem is 5
            (APP_CODE, 3, 5),  # max limit QuotaMaterialCode.NumAttributesPerItem is 5
        ],
        usr_id=mock_app_user_id,
    )
    logging.info("JWT access token generated successfully for smoke test")
    sys.exit(0)


if __name__ == "__main__":
    main()
