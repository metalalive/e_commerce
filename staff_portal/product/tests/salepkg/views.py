import random
import copy
import json
from functools import partial
from unittest.mock import Mock, patch

from django.test import TransactionTestCase
from django.db.models import Count
from django.db.models.constants import LOOKUP_SEP
from django.contrib.auth.models import User as AuthUser
from rest_framework.settings import DEFAULTS as drf_default_settings

from common.util.python import sort_nested_object
from common.util.python.messaging.rpc import RpcReplyEvent
from product.permissions import SaleablePackagePermissions

from product.tests.common import _fixtures, _MockTestClientInfoMixin, assert_view_permission_denied, listitem_rand_assigner, http_request_body_template, assert_view_bulk_create_with_response, SoftDeleteCommonTestMixin
from .common import diff_composite, HttpRequestDataGenSaleablePackage, SaleablePackageVerificationMixin


class SaleablePkgBaseViewTestCase(TransactionTestCase, _MockTestClientInfoMixin, HttpRequestDataGenSaleablePackage):
    permission_class = SaleablePackagePermissions

    def refresh_req_data(self, num_create=None):
        return super().refresh_req_data(fixture_source=_fixtures['ProductSaleablePackage'],
                http_request_body_template=http_request_body_template['ProductSaleablePackage'],
                num_create=num_create)

    def setUp(self):
        _user_info = _fixtures['AuthUser'][0]
        account = AuthUser(**_user_info)
        account.set_password(_user_info['password'])
        account.save()
        # for django app, header name has to start with `HTTP_XXXX`
        self.http_forwarded = self._forwarded_pattern % _user_info['username']
        self._refresh_tags(num=10)
        self._refresh_attrtypes(num=len(_fixtures['ProductAttributeType']))
        self._refresh_saleitems(num=7)

    def tearDown(self):
        self._client.cookies.clear()
        self.min_num_applied_attrs = 0
        self.min_num_applied_tags = 0
        self.min_num_applied_ingredients = 0

    def _send_request_to_backend(self, path, method='post', body=None, expect_shown_fields=None,
            ids=None, extra_query_params=None):
        headers = {'HTTP_FORWARDED': self.http_forwarded,}
        return super()._send_request_to_backend( path=path, method=method, body=body, ids=ids,
                expect_shown_fields=expect_shown_fields, extra_query_params=extra_query_params,
                headers=headers)
## end of class SaleablePkgBaseViewTestCase


class SaleablePkgCreationTestCase(SaleablePkgBaseViewTestCase, SaleablePackageVerificationMixin):
    path = '/saleablepkgs'
    def setUp(self):
        super().setUp()
        self.request_data = self.refresh_req_data(num_create=3)

    def test_permission_denied(self):
        kwargs = { 'testcase':self, 'request_body_data':self.request_data, 'path':self.path,
            'content_type':self._json_mimetype, 'http_forwarded':self.http_forwarded,
            'mock_rpc_path':'product.views.base.SaleablePackageView._usermgt_rpc.get_profile',
            'permission_map': self.permission_class.perms_map['POST'], 'http_method':'post',
            'return_profile_id': self.mock_profile_id[0],
        }
        assert_view_permission_denied(**kwargs)
        kwargs['permission_map'] = self.permission_class.perms_map['PUT']
        kwargs['http_method'] = 'put'
        assert_view_permission_denied(**kwargs)


    @patch('product.views.base.SaleablePackageView._usermgt_rpc.get_profile')
    def test_bulk_ok_with_partial_response(self, mock_get_profile):
        expect_shown_fields  = ['id', 'name', 'visible', 'saleitems_applied',]
        expect_hidden_fields = ['usrprof', 'price', 'tags', 'media_set', 'attributes']
        expect_usrprof = self.mock_profile_id[0]
        mock_get_profile.return_value = self._mock_get_profile(expect_usrprof, 'POST')
        response = assert_view_bulk_create_with_response(testcase=self, expect_shown_fields=expect_shown_fields,
                expect_hidden_fields=expect_hidden_fields, path=self.path, body=self.request_data,
                method='post')
        pkgs_data = sorted(response.json(), key=lambda d:d['id'])
        pkg_ids  = tuple(map(lambda d:d['id'], pkgs_data))
        pkg_objs = self.serializer_class.Meta.model.objects.filter(id__in=pkg_ids).order_by('id')
        pkgs_data_iter = iter(pkgs_data)
        for pkg_obj in pkg_objs:
            pkg_data = next(pkgs_data_iter)
            diff_composite(testcase=self, expect_d=pkg_data['saleitems_applied'], actual_d=pkg_obj,
                lower_elm_name='sale_item', lower_elm_mgr_field='saleitems_applied')


    @patch('product.views.base.SaleablePackageView._usermgt_rpc.get_profile')
    def test_bulk_ok_with_full_response(self, mock_get_profile):
        expect_shown_fields  = ['id', 'name', 'visible', 'saleitems_applied', \
                'usrprof', 'price', 'tags', 'media_set', 'attributes']
        expect_usrprof = self.mock_profile_id[0]
        mock_get_profile.return_value = self._mock_get_profile(expect_usrprof, 'POST')
        response = assert_view_bulk_create_with_response(testcase=self, path=self.path, method='post',
                body=self.request_data, expect_shown_fields=expect_shown_fields, expect_hidden_fields=[] )
        pkgs_data = sorted(response.json(), key=lambda d:d['id'])
        pkg_ids  = tuple(map(lambda d:d['id'], pkgs_data))
        pkg_objs = self.serializer_class.Meta.model.objects.filter(id__in=pkg_ids).order_by('id')
        self.verify_data(actual_data=pkg_objs, expect_data=pkgs_data, usrprof_id=expect_usrprof, verify_id=True)


    @patch('product.views.base.SaleablePackageView._usermgt_rpc.get_profile')
    def test_validation_error_unknown_references(self, mock_get_profile):
        mock_get_profile.return_value = self._mock_get_profile(self.mock_profile_id[1], 'POST')
        invalid_tag_id  = -345
        invalid_unit_id = -346
        invalid_saleitem_id = -347
        edit_data = self.request_data[-1]
        edit_data['tags'].append(invalid_tag_id)
        if any(edit_data['saleitems_applied']):
            item = edit_data['saleitems_applied'][-1]
            item['unit'] = invalid_unit_id
            item['sale_item'] = invalid_saleitem_id
        else:
            item = {'sale_item': invalid_saleitem_id, 'quantity': 1.23, 'unit':invalid_unit_id}
            edit_data['saleitems_applied'].append(item)
        response = self._send_request_to_backend(path=self.path, method='post', body=self.request_data)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        expect_err_msg = 'Invalid pk "%s" - object does not exist.' % invalid_tag_id
        self.assertEqual(expect_err_msg, err_info[-1]['tags'][0])
        expect_err_msg = '"%s" is not a valid choice.' % invalid_unit_id
        self.assertEqual(expect_err_msg, err_info[-1]['saleitems_applied'][-1]['unit'][0])
        expect_err_msg = 'Invalid pk "%s" - object does not exist.' % invalid_saleitem_id
        self.assertEqual(expect_err_msg, err_info[-1]['saleitems_applied'][-1]['sale_item'][0])
## end of class SaleablePkgCreationTestCase


class SaleablePkgUpdateBaseTestCase(SaleablePkgBaseViewTestCase, SaleablePackageVerificationMixin):
    path = '/saleablepkgs'

    def setUp(self):
        num_pkgs = 4
        super().setUp()
        self.expect_usrprof = self.mock_profile_id[0]
        with patch('product.views.base.SaleablePackageView._usermgt_rpc.get_profile') as mock_get_profile:
            mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'POST')
            request_data = self.refresh_req_data(num_create=num_pkgs)
            response = self._send_request_to_backend(path=self.path, method='post',
                     body=request_data, expect_shown_fields=['id', 'name',])
            self.assertEqual(int(response.status_code), 201)
            created_items = response.json()
            created_ids = tuple(map(lambda x:x['id'], created_items))
            self.created_objs = self.serializer_class.Meta.model.objects.filter(id__in=created_ids)
            serializer_ro = self.serializer_class(many=True, instance=self.created_objs)
            self.request_data = serializer_ro.data


class SaleablePkgUpdateTestCase(SaleablePkgUpdateBaseTestCase):
    @patch('product.views.base.SaleablePackageView._usermgt_rpc.get_profile')
    def test_invalid_id(self, mock_get_profile):
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'PUT')
        request_data = self.request_data
        # sub case: lack id
        non_field_error_key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        request_data[0].pop('id',None)
        response = self._send_request_to_backend(path=self.path, method='put', body=request_data)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        err_msg = "cannot be mapped to existing instance, reason: Field 'id' expected a number but got"
        pos = err_info[0][non_field_error_key].find( err_msg )
        self.assertGreater(pos , 0)
        # sub case: invalid data type of id
        request_data[0]['id'] = 99999
        request_data[-1]['id'] = 'string_id'
        response = self._send_request_to_backend(path=self.path, method='put', body=request_data)
        self.assertEqual(int(response.status_code), 403)
        # sub case: mix of valid id and invalid id
        request_data[0]['id'] = 'wrong_id'
        request_data[-1]['id'] = 123
        response = self._send_request_to_backend(path=self.path, method='put', body=request_data)
        self.assertEqual(int(response.status_code), 403)


    @patch('product.views.base.SaleablePackageView._usermgt_rpc.get_profile')
    def test_bulk_ok(self, mock_get_profile):
        expect_shown_fields = ['id', 'name', 'price', 'media_set', 'saleitems_applied']
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'PUT')
        num_rounds = 5
        num_edit = len(self.request_data) >> 1
        edit_data = self.request_data[:num_edit]
        for _ in range(num_rounds):
            self.rand_gen_edit_data(editing_data=edit_data)
            response = self._send_request_to_backend(path=self.path, method='put',
                    body=edit_data,  expect_shown_fields=expect_shown_fields)
            edited_items = response.json()
            self.assertEqual(int(response.status_code), 200)
            fn = lambda x:{key:x[key] for key in expect_shown_fields}
            expect_edited_items = list(map(fn, edit_data))
            actual_edited_items = list(map(fn, edited_items))
            expect_edited_items = sort_nested_object(obj=expect_edited_items)
            actual_edited_items = sort_nested_object(obj=actual_edited_items)
            self.assertListEqual(expect_edited_items, actual_edited_items)
        [obj.refresh_from_db() for obj in self.created_objs]
        self.verify_data(actual_data=self.created_objs[:num_edit], expect_data=edit_data,
                usrprof_id=self.expect_usrprof, verify_id=True)
        self.verify_data(actual_data=self.created_objs[num_edit:], expect_data=self.request_data[num_edit:],
                usrprof_id=self.expect_usrprof, verify_id=True)


    @patch('product.views.base.SaleablePackageView._usermgt_rpc.get_profile')
    def test_different_user_denied(self, mock_get_profile):
        another_usrprof = self.mock_profile_id[1]
        edit_data = self.request_data[:1]
        edit_data[0]['id'] = self.request_data[1]['id']
        mock_get_profile.return_value = self._mock_get_profile(another_usrprof, 'PUT')
        response = self._send_request_to_backend(path=self.path, method='put', body=edit_data,)
        self.assertEqual(int(response.status_code), 403)

    @patch('product.views.base.SaleablePackageView._usermgt_rpc.get_profile')
    def test_conflict_error(self, mock_get_profile):
        key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        edit_data = self.request_data
        discarded_id  = edit_data[0]['id']
        edit_data[0]['id'] = edit_data[1]['id']
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'PUT')
        response = self._send_request_to_backend(path=self.path, method='put', body=edit_data,)
        self.assertEqual(int(response.status_code), 400)
        err_info = response.json()
        err_msg = err_info[key][0]
        pos = err_msg.find('duplicate item found in the list')
        self.assertGreaterEqual(pos, 0)
        pos = err_msg.find(str(discarded_id))
        self.assertLess(pos, 0)
## end of class SaleablePkgUpdateTestCase



class SaleablePkgDeletionTestCase(SaleablePkgUpdateBaseTestCase, SoftDeleteCommonTestMixin):

    @patch('product.views.base.SaleablePackageView._usermgt_rpc.get_profile')
    def test_softdelete_ok(self, mock_get_profile):
        num_delete = len(self.request_data) >> 1
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'DELETE')
        deleted_ids = list(map(lambda x: {'id':x['id']}, self.request_data[:num_delete]))
        response = self._send_request_to_backend(path=self.path, method='delete', body=deleted_ids)
        self.assertEqual(int(response.status_code), 202)
        deleted_ids = list(map(lambda x: x['id'], self.request_data[:num_delete]))
        remain_ids  = list(map(lambda x: x['id'], self.request_data[num_delete:]))
        self.assert_softdelete_items_exist(testcase=self, deleted_ids=deleted_ids, remain_ids=remain_ids,
                model_cls_path='product.models.base.ProductSaleablePackage',)


    def _verify_undeleted_items(self, undeleted_items):
        self.assertGreaterEqual(len(undeleted_items), 1)
        undeleted_items = sorted(undeleted_items, key=lambda d:d['id'])
        undeleted_ids  = tuple(map(lambda d:d['id'], undeleted_items))
        undeleted_objs = self.serializer_class.Meta.model.objects.filter(id__in=undeleted_ids).order_by('id')
        self.verify_data(actual_data=undeleted_objs, expect_data=undeleted_items,
                usrprof_id=self.expect_usrprof, verify_id=True)


    @patch('product.views.base.SaleablePackageView._usermgt_rpc.get_profile')
    def test_undelete_by_time(self, mock_get_profile):
        num_items_original = len(self.request_data)
        remain_items  = self.request_data
        deleted_items = []
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'DELETE')
        model_cls_path = 'product.models.base.ProductSaleablePackage'
        self._softdelete_by_half(remain_items, deleted_items, testcase=self, api_url=self.path,
                model_cls_path=model_cls_path)
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'PATCH')
        while any(deleted_items):
            undeleted_items = self.perform_undelete(testcase=self, path=self.path)
            self._verify_undeleted_items(undeleted_items)
            undeleted_ids  = tuple(map(lambda d:d['id'], undeleted_items))
            moving_gen = tuple(filter(lambda d: d['id'] in undeleted_ids, deleted_items))
            for item in moving_gen:
                remain_items.append(item)
                deleted_items.remove(item)
        self.assertEqual(len(self.request_data), num_items_original)


    @patch('product.views.base.SaleablePackageView._usermgt_rpc.get_profile')
    def test_undelete_specific_item(self, mock_get_profile):
        remain_items  = self.request_data
        deleted_items = []
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'DELETE')
        model_cls_path = 'product.models.base.ProductSaleablePackage'
        self._softdelete_by_half(remain_items, deleted_items, testcase=self, api_url=self.path,
                model_cls_path=model_cls_path)
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'PATCH')
        num_undelete = len(deleted_items) >> 1
        undeleting_items_gen = listitem_rand_assigner(list_=deleted_items, min_num_chosen=num_undelete,
                    max_num_chosen=(num_undelete + 1))
        body = {'ids':[d['id'] for d in undeleting_items_gen]}
        affected_items = self.perform_undelete(body=body, testcase=self, path=self.path)
        self._verify_undeleted_items(affected_items)
        expect_undel_ids = body['ids']
        actual_undel_ids = tuple(map(lambda d:d['id'], affected_items))
        self.assertSetEqual(set(expect_undel_ids), set(actual_undel_ids))
        deleted_items = filter(lambda d:d['id'] not in expect_undel_ids, deleted_items)
        deleted_ids  = tuple(map(lambda d:d['id'], deleted_items))
        deleted_objs = self.serializer_class.Meta.model.objects.get_deleted_set().filter(
                id__in=deleted_ids )
        self.assertEqual(deleted_objs.count(), len(deleted_ids))
        for obj in deleted_objs:
            self.assertTrue(obj.is_deleted())


    @patch('product.views.base.SaleablePackageView._usermgt_rpc.get_profile')
    def test_no_softdeleted_item(self, mock_get_profile):
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'PATCH')
        kwargs = {'testcase': self, 'path':self.path, 'expect_resp_status':410,
                'expect_resp_msg':'Nothing recovered'}
        self.perform_undelete(**kwargs)
        remain_items = self.request_data[:2]
        kwargs['body'] = {'ids':[d['id'] for d in remain_items]}
        self.perform_undelete(**kwargs)

    @patch('product.views.base.SaleablePackageView._usermgt_rpc.get_profile')
    def test_softdelete_permission_denied(self, mock_get_profile):
        another_usrprof = self.mock_profile_id[1]
        mock_get_profile.return_value = self._mock_get_profile(another_usrprof, 'DELETE')
        deleted_ids = list(map(lambda x: {'id':x['id']}, self.request_data))
        response = self._send_request_to_backend(path=self.path, method='delete', body=deleted_ids)
        self.assertEqual(int(response.status_code), 403)

    @patch('product.views.base.SaleablePackageView._usermgt_rpc.get_profile')
    def test_undelete_permission_denied(self, mock_get_profile):
        remain_items  = self.request_data
        deleted_items = []
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'DELETE')
        model_cls_path = 'product.models.base.ProductSaleablePackage'
        self._softdelete_by_half(remain_items, deleted_items, testcase=self, api_url=self.path,
                model_cls_path=model_cls_path)
        self.assertGreater(len(deleted_items), 0)
        another_usrprof = self.mock_profile_id[1]
        mock_get_profile.return_value = self._mock_get_profile(another_usrprof, 'PATCH', access_control=False)
        kwargs = {'testcase': self, 'path':self.path, 'expect_resp_status':403,
                'expect_resp_msg':'You do not have permission to perform this action.'}
        kwargs['body'] = {'ids':[d['id'] for d in deleted_items]}
        self.perform_undelete(**kwargs)
## end of class SaleablePkgDeletionTestCase


class SaleablePkgQueryTestCase(SaleablePkgUpdateBaseTestCase):
    @patch('product.views.base.SaleablePackageView._usermgt_rpc.get_profile')
    def test_order(self, mock_get_profile):
        order_field = 'price'
        expect_shown_fields = ['id','name','price']
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'GET')
        extra_query_params = {'ordering': order_field}
        response = self._send_request_to_backend(path=self.path, method='get',
                expect_shown_fields=expect_shown_fields, extra_query_params=extra_query_params)
        actual_items = response.json()
        expect_items = self.serializer_class.Meta.model.objects.order_by(order_field).values(*expect_shown_fields)
        actual_items = json.dumps(actual_items)
        expect_items = json.dumps(list(expect_items))
        if actual_items != expect_items:
            import pdb
            pdb.set_trace()
        self.assertEqual(actual_items , expect_items)


class SaleablePkgAdvancedSearchTestCase(SaleablePkgUpdateBaseTestCase):
    min_num_applied_tags = 2

    def _test_advanced_search_common(self, adv_cond):
        extra_query_params = {'advanced_search': 'yes', 'adv_search_cond': json.dumps(adv_cond)}
        response = self._send_request_to_backend(path=self.path, method='get',
                extra_query_params=extra_query_params)
        actual_items = response.json()
        self.assertEqual(int(response.status_code), 200)
        self.assertGreaterEqual(len(actual_items), 1)
        return actual_items

    @patch('product.views.base.SaleablePackageView._usermgt_rpc.get_profile')
    def test_saleitems_applied(self, mock_get_profile):
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'GET')
        qset = self.serializer_class.Meta.model.objects.annotate(num_compo=Count('saleitems_applied__sale_item'))
        qset = qset.filter(num_compo__gt=0)
        values = qset.values('id','saleitems_applied')
        limit = max(values.count() >> 1, 1)
        values = values[:limit]
        adv_cond = {'operator': 'or', 'operands': []}
        for value in values:
            adv_cond_sub_clause = {
                'operator': 'and',
                'operands': [
                    {
                        'operator':'==',
                        'operands':['saleitems_applied__sale_item', value['saleitems_applied__sale_item_id']],
                        'metadata':{}
                    },
                    {
                        'operator':'==',
                        'operands':['saleitems_applied__package', value['id']],
                        'metadata':{}
                    },
                ]
            }
            adv_cond['operands'].append(adv_cond_sub_clause)
        pkgs_found = self._test_advanced_search_common(adv_cond=adv_cond)
        expect_pkgs_found = list(dict.fromkeys([v['id'] for v in values]))
        actual_pkgs_found = list(map(lambda v:v['id'], pkgs_found))
        self.assertListEqual(sorted(actual_pkgs_found), sorted(expect_pkgs_found))


    @patch('product.views.base.SaleablePackageView._usermgt_rpc.get_profile')
    def test_complex_condition(self, mock_get_profile):
        mock_get_profile.return_value = self._mock_get_profile(self.expect_usrprof, 'GET')
        aggregates_nested_fields = {'num_compo': Count('saleitems_applied__sale_item'),
                'num_tags': Count('tags'), }
        filter_nested_fields = {'num_tags__gte':2 , 'num_compo__gt':0}
        qset = self.serializer_class.Meta.model.objects.annotate(**aggregates_nested_fields)
        qset = qset.filter(**filter_nested_fields)
        salepkg = qset.first()
        composite = salepkg.saleitems_applied.first()
        expect_saleitem_id = composite.sale_item.id
        expect_tag_ids = salepkg.tags.values_list('id', flat=True)
        adv_cond = {
            'operator': 'and',
            'operands': [
                {
                    'operator':'==',
                    'operands':['saleitems_applied__sale_item', expect_saleitem_id],
                    'metadata':{}
                },
                {
                    'operator':'or',
                    'operands': [
                        {
                            'operator':'==',
                            'operands':['tags__id', expect_tag_ids[0]],
                            'metadata':{}
                        },
                        {
                            'operator':'==',
                            'operands':['tags__id', expect_tag_ids[1]],
                            'metadata':{}
                        },
                    ]
                }
            ]
        }
        pkgs_found = self._test_advanced_search_common(adv_cond=adv_cond)
        actual_ids = list(map(lambda d:d['id'] , pkgs_found))
        self.assertIn(salepkg.id, actual_ids)
        for found_item in pkgs_found:
            diff = set(found_item['tags']) - set(list(expect_tag_ids))
            self.assertLess(len(diff), len(found_item['tags']))
            actual_saleitem_ids = tuple(map(lambda x:x['sale_item'], found_item['saleitems_applied']))
            self.assertIn(expect_saleitem_id, actual_saleitem_ids)
## end of class SaleablePkgAdvancedSearchTestCase

