import os
from pathlib import Path
import unittest

from user_management.util import render_mail_content

app_basepath = Path(os.environ["APP_BASE_PATH"]).resolve(strict=True)


class RenderMailContentTestCase(unittest.TestCase):
    def setUp(self):
        self._example_path = app_basepath.joinpath("tests/examples")

    def test_ok(self):
        msg_template_path = self._example_path.joinpath("mail_content_template.html")
        subject_template_path = self._example_path.joinpath(
            "mail_subject_template.html"
        )
        msg_data = {
            "func_name": "user-preference-history",
            "syntax": "zero-sized type",
            "module_name": "neural-network-training",
        }
        subject_data = {"someone_else": "neighbor", "thing": "grass", "adj": "greener"}
        actual_content, actual_subject = render_mail_content(
            msg_template_path=msg_template_path,
            subject_template_path=subject_template_path,
            msg_data=msg_data,
            subject_data=subject_data,
        )
        self.assertEqual(actual_subject, "Your neighbor's grass always looks greener")
        self.assertEqual(actual_content.index("user-preference-history"), 1)
        self.assertGreater(
            actual_content.index("added to the neural-network-training module"), 0
        )
        self.assertGreater(
            actual_content.index("zero-sized type could lead to undefined behaviour"), 0
        )

    def test_template_not_exists(self):
        msg_template_path = self._example_path.joinpath("xxxx.html")
        subject_template_path = self._example_path.joinpath(
            "mail_subject_template.html"
        )
        msg_data = {}
        subject_data = {"someone_else": "neighbor", "thing": "grass", "adj": "greener"}
        with self.assertRaises(FileNotFoundError):
            render_mail_content(
                msg_template_path=msg_template_path,
                subject_template_path=subject_template_path,
                msg_data=msg_data,
                subject_data=subject_data,
            )

    def test_render_error(self):
        msg_template_path = self._example_path.joinpath("mail_content_template.html")
        subject_template_path = self._example_path.joinpath(
            "mail_subject_template.html"
        )
        # django template simply ignores the missing variables
        msg_data = {"func_name": "lifetime-ann\x00tate", "syntax": "non-leak"}
        subject_data = {
            "someone_else": "brother",
            "thing": "food",
        }  # will cause key error
        with self.assertRaises(KeyError) as ae:
            render_mail_content(
                msg_template_path=msg_template_path,
                subject_template_path=subject_template_path,
                msg_data=msg_data,
                subject_data=subject_data,
            )
        assert "adj" in ae.exception.args
