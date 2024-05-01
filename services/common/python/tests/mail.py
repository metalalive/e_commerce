import os
import unittest
from pathlib import Path
from smtplib import SMTPAuthenticationError

from ecommerce_common.logging.logger import ExtendedLogger
from ecommerce_common.util.async_tasks import send_email

srv_basepath = Path(os.environ["SERVICE_BASE_PATH"]).resolve(strict=True)


class SendEmailTestCase(unittest.TestCase):
    def setUp(self):
        self._cert_fullpath = srv_basepath.joinpath(
            "common/python/tests/gmail-crt-chain.pem"
        )
        pass

    def tearDown(self):
        pass

    def test_login_failure(self):
        mock_secret = {
            "host": "smtp.gmail.com",
            "port": 587,
            "username": "civilized@gmail.com",
            "password": "barbarian",
            "cert_path": self._cert_fullpath,
        }  # note cert_path is optional
        with self.assertRaises((SMTPAuthenticationError,)):
            send_email(
                secret=mock_secret,
                attachment_paths=[],
                subject="locally equivalent",
                body="AccessPoint (AP) infrastructure mode",
                sender="really-very-easy@trival.org",
                recipients=["LightingAlien@buried-myth123.io"],
            )
        # auth failure is expected, this test case does not provide
        # real credential
