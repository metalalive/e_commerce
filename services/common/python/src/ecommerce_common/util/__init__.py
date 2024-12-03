import os
import sys
import json
import string
import logging
from pathlib import Path
from importlib import import_module
from collections.abc import Iterable


def get_header_name(name: str):
    """
    translate given key in Django request.META to header name in HTTP form
    """
    from django.http.request import HttpHeaders

    prefix = HttpHeaders.HTTP_PREFIX
    if name.startswith(prefix):  # skip those in HttpHeaders.UNPREFIXED_HEADERS
        name = name[len(prefix) :]
    out = name.replace("_", "-").lower()
    # print("get_header_name, out : "+ str(out))
    return out


def get_request_meta_key(header_name: str):
    """
    translate given header name in HTTP form to accessible key in Django request.META
    """
    from django.http.request import HttpHeaders

    out = header_name.replace("-", "_").upper()
    if out not in HttpHeaders.UNPREFIXED_HEADERS:
        out = "%s%s" % (HttpHeaders.HTTP_PREFIX, out)
    return out


def serial_kvpairs_to_dict(serial_kv: str, delimiter_pair=",", delimiter_kv=":"):
    outlst = []
    outdict = {}
    kv_pairs = serial_kv.split(delimiter_pair)
    for kv in kv_pairs:
        if delimiter_kv in kv:
            kv_lst = kv.split(delimiter_kv, 1)
            kv_lst = list(map(str.strip, kv_lst))
            outlst.append(kv_lst)
    # print('outlst = %s' % outlst)
    for pair in outlst:
        key = pair[0]
        if outdict.get(key, None) is None:
            outdict[key] = [pair[1]]
        else:
            outdict[key].append(pair[1])
    return outdict


def accept_mimetypes_lookup(http_accept: str, expected_types):
    # note that this is case-insensitive lookup
    client_accept = http_accept.split(",")
    client_accept = list(map(lambda x: x.strip().lower(), client_accept))
    expected_types = list(map(lambda x: x.strip().lower(), expected_types))
    result = filter(lambda x: x in expected_types, client_accept)
    return list(result)


def _get_amqp_url(secrets_path, idx=0):
    # use rabbitmqctl to manage accounts
    # TODO, dev / test environment isolation
    secrets = None
    with open(secrets_path, "r") as f:
        secrets = json.load(f)
        secrets = secrets["amqp_broker"]
        secrets = secrets[
            idx
        ]  # the default is to select index 0 as guest account (with password)
    assert secrets, "failed to load secrets from file"
    protocol = secrets["protocol"]
    username = secrets["username"]
    passwd = secrets["password"]
    host = secrets["host"]
    port = secrets["port"]
    out = "%s://%s:%s@%s:%s" % (protocol, username, passwd, host, port)
    return out


def merge_partial_dup_listitem(list_in, combo_key, merge_keys):
    inter_dict = {}
    for old_item in list_in:
        inter_key = tuple([old_item.pop(k) for k in combo_key])
        for mk in merge_keys:
            if inter_dict.get(inter_key, None) is None:
                inter_dict[inter_key] = {}
            if inter_dict[inter_key].get(mk, None) is None:
                inter_dict[inter_key][mk] = []
            value = old_item.pop(mk, None)
            if value:
                inter_dict[inter_key][mk].append(value)
    list_in.clear()
    for inter_key, inter_value in inter_dict.items():
        new_item = {}
        for idx in range(len(inter_key)):
            new_item[combo_key[idx]] = inter_key[idx]
        for mk, merged_value in inter_value.items():
            new_item[mk] = merged_value
        list_in.append(new_item)


def string_unprintable_check(value: str, extra_unprintable_set=None):
    """
    this function returns any unprintable characters in the given
    input string, application callers can optionally add more characters
    which are not allowed to print in their usage context.
    """
    extra_unprintable_set = extra_unprintable_set or []
    _printable_char_set = set(string.printable) - set(extra_unprintable_set)
    generator = filter(lambda x: not x in _printable_char_set, value)
    return list(generator)


def load_config_to_module(cfg_path, module_path):
    _module = sys.modules[module_path]
    data = None
    with open(cfg_path, "r") as f:
        data = json.load(f)
    assert data, "failed to load configuration from file %s" % cfg_path
    for key in _module.__dict__.keys():
        if data.get(key, None) is None:
            continue
        setattr(_module, key, data[key])


def import_module_string(dotted_path: str):
    """
    Import a dotted module path and return the attribute/class designated by the
    last name in the path. Raise ImportError if the import failed.
    """
    try:
        module_path, class_name = dotted_path.rsplit(".", 1)
    except ValueError as err:
        raise ImportError("%s doesn't look like a module path" % dotted_path) from err

    module = import_module(module_path)
    try:
        return getattr(module, class_name)
    except AttributeError as err:
        raise ImportError(
            'Module "%s" does not define a "%s" attribute/class'
            % (module_path, class_name)
        ) from err


def get_credential_from_secrets(secret_map: dict, base_path: Path, secret_path: str):
    """example of a database section within a secret file
    db_credentials = {
        'role_dba'           : secrets['backend_apps']['databases']['site_dba'] ,
        'role_auth_service'  : secrets['backend_apps']['databases']['usermgt_service'] ,
        'file_upload_service': secrets['backend_apps']['databases']['file_upload_service'] ,
    }
    """
    _credentials = {}
    secret_fullpath = base_path.joinpath(secret_path)
    with open(secret_fullpath) as f:
        secrets = json.load(f)
        node = None
        for k, v in secret_map.items():
            node = secrets
            hierarchy = v.split(".")
            for v2 in hierarchy:
                node = node.get(v2, None)
            _credentials[k] = node
    return _credentials


def flatten_nested_iterable(list_):
    for item in list_:
        if isinstance(item, Iterable) and not isinstance(item, (str, bytes)):
            yield from flatten_nested_iterable(item)
        else:
            yield item


def _sort_nested_object(obj, key_fn_list=None, key_fn_dict=None):
    # note this function recursively converts nested dictionary fields to a list of
    # key-value pairs, which is essential during sorting operations because `dict`
    # type does not support comparison operators like `>` or `<`
    if isinstance(obj, dict):
        src = [[k, _sort_nested_object(v)] for k, v in obj.items()]  # DO NOT use tuple
        args = [src]
        if key_fn_dict and callable(key_fn_dict):
            args.append(key_fn_dict)
        sorted_src = sorted(*args)
        src = ExtendedList(from_dict=True)
        src.extend(sorted_src)
        return src
    elif isinstance(obj, list):
        src = [_sort_nested_object(x) for x in obj]
        args = [src]
        if key_fn_list and callable(key_fn_list):
            args.append(key_fn_list)
        return sorted(*args)
    else:
        return obj


def _sort_nested_object_post_process(obj):
    if isinstance(obj, list):
        src = [_sort_nested_object_post_process(x) for x in obj]
        if isinstance(obj, ExtendedList) and obj.from_dict:
            src = dict(src)
        return src
    else:
        return obj


def sort_nested_object(obj, key_fn_list=None, key_fn_dict=None):
    sorted_obj = _sort_nested_object(
        obj=obj, key_fn_list=key_fn_list, key_fn_dict=key_fn_dict
    )
    return _sort_nested_object_post_process(obj=sorted_obj)


class ExtendedList(list):
    def __init__(self, *args, from_dict=False, **kwargs):
        self._converted_from_dict = from_dict
        super().__init__(*args, **kwargs)

    @property
    def from_dict(self):
        return self._converted_from_dict


class ExtendedDict(dict):
    def __init__(self, *args, **kwargs):
        self._modified = False
        super().__init__(*args, **kwargs)

    def __setitem__(self, key, value):
        self._modified = True
        super().__setitem__(key, value)

    @property
    def modified(self):
        return self._modified

    def update(self, new_kwargs, overwrite=True):
        if overwrite is False:
            common_keys = set(self.keys()) & set(new_kwargs.keys())
            new_kwargs = {k: v for k, v in new_kwargs.items() if k not in common_keys}
        if any(new_kwargs):
            self._modified = True
        return super().update(new_kwargs)


## end of class ExtendedDict


# class inheritance doesn't seem to work in metaclass
class BaseLookupMeta(type):
    def getitem(cls, key, _dict_name):
        item = None
        try:
            _dict = getattr(cls, _dict_name)
            item = _dict[key]
        except (KeyError, AttributeError) as e:
            err_msg = "[%s] %s when searching in %s, no such key : %s" % (
                cls.__name__,
                type(e),
                _dict,
                key,
            )
            print(err_msg)  # TODO, log to local file system
        return item


class BaseUriLookup(metaclass=BaseLookupMeta):
    _urls = {}

    def __getitem__(cls, key):
        return type(cls).getitem(
            key, "_urls"
        )  # first argument implicitly sent by type(cls)

    def __iter__(cls):
        cls._iter_api_url = iter(cls._urls)
        return cls

    def __next__(cls):
        key = next(cls._iter_api_url)
        return (key, cls._urls[key])


class BaseTemplateLookup(metaclass=BaseLookupMeta):
    _template_names = {}
    template_path = ""

    def __getitem__(cls, key):
        item = BaseLookupMeta.getitem(cls, key, "_template_names")
        if item is not None:
            if isinstance(item, str):
                item = os.path.join(cls.template_path, item)
            elif isinstance(item, list):
                item = [os.path.join(cls.template_path, x) for x in item]
        return item
