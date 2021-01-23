import json

from django.conf  import  settings as django_settings
from django.http.request    import HttpHeaders
from django.core.exceptions import ImproperlyConfigured


def get_fixture_pks(filepath:str, pkg_hierarchy:str):
    assert pkg_hierarchy, "pkg_hierarchy must be fully-qualified model path"
    preserved = None
    for d in django_settings.FIXTURE_DIRS:
        fixture_path = '/'.join([d, filepath])
        with open(fixture_path, 'r') as f:
            preserved = json.load(f)
        if preserved:
            break
    if not preserved:
        raise ImproperlyConfigured("fixture file not found, recheck FIXTURE_DIRS in settings.py")
    return  [str(item['pk']) for item in preserved if item['model'] == pkg_hierarchy]


def get_header_name(name:str):
    """
    normalize given input to valid header name that can be placed in HTTP request
    """
    prefix = HttpHeaders.HTTP_PREFIX
    assert name.startswith(prefix),  "{} should have prefix : {}".format(name, prefix)
    out = name[len(prefix):]
    out = out.replace('_', '-')
    # print("get_header_name, out : "+ str(out))
    return out


