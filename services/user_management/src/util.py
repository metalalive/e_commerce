from pathlib import Path
from typing import Dict, Optional, Union

from django.template import Template, Context
from django.utils.html import strip_tags


def render_mail_content(
    msg_template_path: Union[str, Path],
    msg_data: Dict[str, str],
    subject_template_path: Union[str, Path],
    subject_data: Optional[Dict[str, str]] = None,
) -> tuple[str, str]:
    msg_html, subject = None, None
    with open(subject_template_path, "r") as f:
        # load template files, render with given data
        template = f.readline()
        data = subject_data or {}
        subject = template.rstrip("\n").format(**data)
    with open(msg_template_path, "r") as f:
        template = Template(f.read())
        context = Context(msg_data)
        renderred = template.render(context)
        msg_html = strip_tags(renderred)
    assert subject, "Error occured on reading file %s" % subject_template_path
    assert msg_html, "Error occured on reading file %s" % msg_template_path
    return (msg_html, subject)
