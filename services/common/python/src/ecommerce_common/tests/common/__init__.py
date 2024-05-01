import os
import random
import json
import copy
import operator
from datetime import datetime, timedelta
from functools import partial, reduce

from ecommerce_common.util import ExtendedList, import_module_string
from ecommerce_common.auth.keystore import (
    create_keystore_helper,
    JWKSFilePersistHandler,
)


def rand_gen_request_body(customize_item_fn, data_gen, template=None):
    template = template or {}

    def rand_gen_single_req(data):
        single_req_item = copy.deepcopy(template)
        single_req_item.update(data)
        customize_item_fn(single_req_item)
        return single_req_item

    return map(rand_gen_single_req, data_gen)


def listitem_rand_assigner(
    list_, min_num_chosen: int = 2, max_num_chosen: int = -1, distinct: bool = True
):
    # utility for testing
    assert any(list_), "input list should not be empty"
    assert min_num_chosen >= 0, "min_num_chosen = %s" % min_num_chosen
    num_avail = len(list_)
    if max_num_chosen > 0:
        err_msg = "max_num_chosen = %s, min_num_chosen = %s" % (
            max_num_chosen,
            min_num_chosen,
        )
        assert max_num_chosen > min_num_chosen, err_msg
        if max_num_chosen > (num_avail + 1) and distinct is True:
            err_msg = "num_avail = %s, max_num_chosen = %s, distinct = %s" % (
                num_avail,
                max_num_chosen,
                distinct,
            )
            raise ValueError(err_msg)
    else:
        err_msg = "num_avail = %s , min_num_chosen = %s" % (num_avail, min_num_chosen)
        assert num_avail >= min_num_chosen, err_msg
        max_num_chosen = num_avail + 1
    if distinct:
        list_ = list(list_)
    num_assigned = random.randrange(min_num_chosen, max_num_chosen)
    for _ in range(num_assigned):
        idx = random.randrange(num_avail)
        yield list_[idx]
        if distinct:
            num_avail -= 1
            del list_[idx]


## end of listitem_rand_assigner


def capture_error(testcase, err_cls, exe_fn, exe_kwargs=None):
    error_caught = None
    with testcase.assertRaises(err_cls):
        try:
            exe_kwargs = exe_kwargs or {}
            exe_fn(**exe_kwargs)
        except err_cls as e:
            error_caught = e
            raise
    testcase.assertIsNotNone(error_caught)
    return error_caught


class HttpRequestDataGen:
    rand_create = True

    def customize_req_data_item(self, item, **kwargs):
        raise NotImplementedError()

    def refresh_req_data(
        self, fixture_source, http_request_body_template, num_create=None
    ):
        if self.rand_create:
            kwargs = {"list_": fixture_source}
            if num_create:
                kwargs.update(
                    {"min_num_chosen": num_create, "max_num_chosen": (num_create + 1)}
                )
            data_gen = listitem_rand_assigner(**kwargs)
        else:
            num_create = num_create or len(fixture_source)
            data_gen = iter(fixture_source[:num_create])
        out = rand_gen_request_body(
            customize_item_fn=self.customize_req_data_item,
            data_gen=data_gen,
            template=http_request_body_template,
        )
        return list(out)


class TreeNodeMixin:
    def __init__(self, value=None):
        self.value = value
        self._parent = None
        self.children = []

    @property
    def depth(self):
        num_list = [t.depth for t in self.children]
        max_depth = max(num_list) if any(num_list) else 0
        return 1 + max_depth

    @property
    def num_nodes(self):
        num_list = [t.num_nodes for t in self.children]
        num_descs = reduce(operator.add, num_list) if any(num_list) else 0
        return 1 + num_descs

    @property
    def parent(self):
        return self._parent

    @parent.setter
    def parent(self, new_parent):
        old_parent = self._parent
        if old_parent:
            old_parent.children.remove(self)
        if new_parent:
            new_parent.children.append(self)
        self._parent = new_parent

    @classmethod
    def rand_gen_trees(
        cls,
        num_trees,
        min_num_nodes=2,
        max_num_nodes=15,
        min_num_siblings=1,
        max_num_siblings=4,
        write_value_fn=None,
    ):
        # this method will generate number of trees, each tree has random number of nodes,
        # each non-leaf node has at least one child (might be random number of children)
        trees = ExtendedList()
        for _ in range(num_trees):
            tree = [
                cls()
                for _ in range(random.randrange(min_num_nodes, (max_num_nodes + 1)))
            ]
            if write_value_fn and callable(write_value_fn):
                for node in tree:
                    write_value_fn(node)
            parent_iter = iter(tree)
            child_iter = iter(tree)
            next(child_iter)
            try:
                for curr_parent in parent_iter:
                    num_siblings = random.randrange(
                        min_num_siblings, (max_num_siblings + 1)
                    )
                    # curr_parent.children = []
                    for _ in range(num_siblings):
                        curr_child = next(child_iter)
                        curr_child.parent = curr_parent
            except StopIteration:
                pass
            finally:
                trees.append(tree[0])
        return trees

    @classmethod
    def gen_from_closure_data(
        cls, entity_data, closure_data, custom_value_setup_fn=None
    ):
        tmp_nodes = {}
        nodes_data = closure_data.filter(depth=0)  # tightly coupled with Django ORM
        for node_data in nodes_data:
            assert node_data["ancestor"] == node_data["descendant"], (
                "depth is zero, ancestor and descendant \
                    have to be the same, node data: %s"
                % node_data
            )
            node = tmp_nodes.get(node_data["ancestor"])
            assert node is None, "node conflict, depth:0, node data: %s" % node_data
            entity_dataitem = entity_data.get(id=node_data["ancestor"])
            if custom_value_setup_fn and callable(custom_value_setup_fn):
                entity_dataitem = custom_value_setup_fn(entity_dataitem)
            node_instance = cls(value=entity_dataitem)
            tmp_nodes[node_data["ancestor"]] = node_instance

        nodes_data = closure_data.filter(depth=1)
        for node_data in nodes_data:
            assert node_data["ancestor"] != node_data["descendant"], (
                "depth is non-zero, ancestor and \
                    descendant have to be different, node data: %s"
                % node_data
            )
            parent_node = tmp_nodes[node_data["ancestor"]]
            child_node = tmp_nodes[node_data["descendant"]]
            assert (parent_node is not None) and (child_node is not None), (
                "both of parent and child must not be null, node data: %s" % node_data
            )
            assert (child_node.parent is None) and (
                child_node not in parent_node.children
            ), ("path duplicate ? depth:1, node data: %s" % node_data)
            child_node.parent = parent_node

        nodes_data = closure_data.filter(depth__gte=2)
        for node_data in nodes_data:
            assert node_data["ancestor"] != node_data["descendant"], (
                "depth is non-zero, ancestor and \
                    descendant have to be different, node data: %s"
                % node_data
            )
            asc_node = tmp_nodes[node_data["ancestor"]]
            desc_node = tmp_nodes[node_data["descendant"]]
            assert (asc_node is not None) and (desc_node is not None), (
                "both of ancestor and decendant must not be null, node data: %s"
                % node_data
            )
            curr_node_pos = desc_node
            for _ in range(node_data["depth"]):
                curr_node_pos = curr_node_pos.parent
            # if curr_node_pos != asc_node:
            #    import pdb
            #    pdb.set_trace()
            assert curr_node_pos == asc_node, (
                "corrupted closure node data: %s" % node_data
            )

        trees = ExtendedList()
        flattened_nodes = list(tmp_nodes.values())
        trees.extend(list(filter(lambda t: t.parent is None, flattened_nodes)))
        trees.entity_data = entity_data
        trees.closure_data = closure_data
        return trees

    ## end of gen_from_closure_data

    @classmethod
    def _compare_single_tree(cls, node_a, node_b, value_compare_fn):
        diff = []
        is_the_same = value_compare_fn(val_a=node_a.value, val_b=node_b.value)
        if is_the_same is False:
            item = {
                "message": "value does not matched",
                "value_a": node_a.value,
                "value_b": node_b.value,
            }
            diff.append(item)
        num_child_a = len(node_a.children)
        num_child_b = len(node_b.children)
        if num_child_a == num_child_b:
            if num_child_a > 0:
                _, not_matched = cls.compare_trees(
                    trees_a=node_a.children,
                    trees_b=node_b.children,
                    value_compare_fn=value_compare_fn,
                )
                diff.extend(not_matched)
            else:
                pass  # leaf node, end of recursive call
        else:
            item = {
                "message": "num of children does not matched",
                "value_a": num_child_a,
                "value_b": num_child_b,
            }
            diff.append(item)
        return diff

    @classmethod
    def compare_trees(cls, trees_a, trees_b, value_compare_fn):
        assert callable(value_compare_fn), (
            "value_compare_fn: %s has to be callable" % value_compare_fn
        )
        matched = []
        not_matched = []
        for tree_a in trees_a:
            matched_tree = None
            diffs = []
            for tree_b in trees_b:
                diff = cls._compare_single_tree(
                    node_a=tree_a, node_b=tree_b, value_compare_fn=value_compare_fn
                )
                if any(diff):
                    diffs.append(diff)
                else:
                    matched_tree = tree_b
                    break
            if matched_tree is not None:
                matched.append((tree_a, matched_tree))
            else:
                item = {
                    "message": "tree_a does not matched",
                    "tree_a": tree_a.value,
                    "diffs": diffs,
                }
                not_matched.append(item)
        # if any(not_matched):
        #    import pdb
        #    pdb.set_trace()
        return matched, not_matched


## end of class TreeNodeMixin


def _setup_keyfile(filepath):
    dir_tear_down = False
    file_tear_down = False
    dir_path = os.path.dirname(filepath)
    if not os.path.exists(dir_path):
        dir_tear_down = True
        os.makedirs(dir_path, exist_ok=False)
    if not os.path.exists(filepath):
        file_tear_down = True
        with open(filepath, "wb") as f:
            empty_json_obj = "%s%s" % (
                JWKSFilePersistHandler.JSONFILE_START_LINE,
                JWKSFilePersistHandler.JSONFILE_END_LINE,
            )
            f.write(empty_json_obj.encode())
    return dir_tear_down, file_tear_down


def _teardown_keyfile(filepath, del_dir: bool, del_file: bool):
    dir_path = os.path.dirname(filepath)
    if del_file:
        os.remove(filepath)
    if del_dir:
        shutil.rmtree(dir_path)


class KeystoreMixin:
    _keystore_init_config = None
    ## alwasy use `JWKSFilePersistHandler` to run the test cases , for example :
    ## {
    ##     'keystore': 'common.auth.keystore.BaseAuthKeyStore',
    ##     'persist_secret_handler': {
    ##         'module_path': 'common.auth.keystore.JWKSFilePersistHandler',
    ##         'init_kwargs': {
    ##             'filepath': './tmp/cache/service_name/test_or_dev/jwks/privkey/current.json',
    ##             'name':'secret', 'expired_after_days': 7, 'flush_threshold':4,
    ##         },
    ##     },
    ##     'persist_pubkey_handler': {
    ##         'module_path': 'common.auth.keystore.JWKSFilePersistHandler',
    ##         'init_kwargs': {
    ##             'filepath': './tmp/cache/service_name/test_or_dev/jwks/pubkey/current.json',
    ##             'name':'pubkey', 'expired_after_days': 9, 'flush_threshold':4,
    ##         },
    ##     },
    ## }

    def _setup_keystore(self):
        from ecommerce_common.auth.jwt import JwkRsaKeygenHandler

        persist_labels = ("persist_pubkey_handler", "persist_secret_handler")
        self.tear_down_files = {}
        for label in persist_labels:
            filepath = self._keystore_init_config[label]["init_kwargs"]["filepath"]
            del_dir, del_file = _setup_keyfile(filepath=filepath)
            item = {"del_dir": del_dir, "del_file": del_file, "filepath": filepath}
            self.tear_down_files[label] = item
        num_keys = self._keystore_init_config["persist_secret_handler"]["init_kwargs"][
            "flush_threshold"
        ]
        keystore = create_keystore_helper(
            cfg=self._keystore_init_config, import_fn=import_module_string
        )
        keygen_handler = JwkRsaKeygenHandler()
        result = keystore.rotate(
            keygen_handler=keygen_handler,
            key_size_in_bits=2048,
            num_keys=num_keys,
            date_limit=None,
        )
        self._keys_metadata = result["new"]
        self._keystore = keystore

    def _teardown_keystore(self):
        for item in self.tear_down_files.values():
            _teardown_keyfile(
                filepath=item["filepath"],
                del_dir=item["del_dir"],
                del_file=item["del_file"],
            )

    def _access_token_serialize_auth_info(self, audience, profile, superuser_status=1):
        role_extract_fn = lambda d: {key: d[key] for key in ("app_code", "codename")}
        quota_extract_fn = lambda d: {
            key: d[key] for key in ("app_code", "mat_code", "maxnum")
        }
        out = {
            "id": profile["id"],
            "priv_status": profile["privilege_status"],
            "perms": list(map(role_extract_fn, profile["roles"])),
            "quota": list(map(quota_extract_fn, profile["quotas"])),
        }
        if not any(out["perms"]) and out["priv_status"] != superuser_status:
            errmsg = "the user does not have access to these resource services listed in audience field"
            raise ValueError(errmsg)
        return out

    def gen_access_token(
        self, profile, audience, ks_cfg=None, access_token_valid_seconds=300
    ):
        from ecommerce_common.auth.jwt import JWT

        ks_cfg = ks_cfg or self._keystore_init_config
        profile_serial = self._access_token_serialize_auth_info(
            audience=audience, profile=profile
        )
        keystore = create_keystore_helper(cfg=ks_cfg, import_fn=import_module_string)
        now_time = datetime.utcnow()
        expiry = now_time + timedelta(seconds=access_token_valid_seconds)
        token = JWT()
        payload = {
            "profile": profile_serial.pop("id"),
            "aud": audience,
            "iat": now_time,
            "exp": expiry,
        }
        payload.update(profile_serial)  # roles, quota
        token.payload.update(payload)
        return token.encode(keystore=keystore)

    def _mocked_get_jwks(self):
        from ecommerce_common.auth.jwt import stream_jwks_file

        filepath = self._keystore_init_config["persist_pubkey_handler"]["init_kwargs"][
            "filepath"
        ]
        full_response_body = list(stream_jwks_file(filepath=filepath))
        full_response_body = "".join(full_response_body)
        return json.loads(full_response_body)
