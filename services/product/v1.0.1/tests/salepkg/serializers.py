import random
import copy
import json
from functools import partial
from unittest.mock import Mock

from django.test import TransactionTestCase
from rest_framework.exceptions import ValidationError as DRFValidationError
from rest_framework.settings import DEFAULTS as drf_default_settings

from ecommerce_common.validators  import NumberBoundaryValidator
from tests.common import _fixtures, http_request_body_template, listitem_rand_assigner, rand_gen_request_body, assert_field_equal
from .common import diff_composite, HttpRequestDataGenSaleablePackage, SaleablePackageVerificationMixin

class SaleablePkgCommonMixin(HttpRequestDataGenSaleablePackage, SaleablePackageVerificationMixin):
    num_users = 1
    min_num_pkgs  = 2

    def setUp(self):
        self._refresh_tags(num=10)
        self._refresh_attrtypes(num=len(_fixtures['ProductAttributeType']))
        self._refresh_saleitems(num=7)
        self.profile_ids = [random.randrange(1,15) for _ in range(self.num_users)]
        salepkgs_data_gen = listitem_rand_assigner(list_=_fixtures['ProductSaleablePackage'],
                min_num_chosen=self.min_num_pkgs)
        request_data = rand_gen_request_body(customize_item_fn=self.customize_req_data_item,
                data_gen=salepkgs_data_gen,  template=http_request_body_template['ProductSaleablePackage'])
        self.request_data = list(request_data)


class SaleablePkgCreationTestCase(SaleablePkgCommonMixin, TransactionTestCase):
    def setUp(self):
        super().setUp()
        self.serializer_kwargs = {'data': copy.deepcopy(self.request_data),
                'many': True, 'usrprof_id': self.profile_ids[0],}

    def test_bulk_ok(self):
        serializer = self.serializer_class( **self.serializer_kwargs )
        serializer.is_valid(raise_exception=True)
        actual_instances = serializer.save()
        self.verify_data(actual_data=actual_instances, expect_data=self.request_data,
                usrprof_id=self.profile_ids[0])

    def test_skip_given_id(self):
        invalid_cases = (12,)
        self.serializer_kwargs['data'][0]['id'] = invalid_cases[0]
        self.assertEqual(self.serializer_kwargs['data'][0]['id'] , invalid_cases[0])
        serializer = self.serializer_class( **self.serializer_kwargs )
        serializer.is_valid(raise_exception=True)
        with self.assertRaises(KeyError):
            validated_id = serializer.validated_data[0]['id']
            self.assertEqual(validated_id , invalid_cases[0])
        with self.assertRaises(KeyError):
            validated_id = self.serializer_kwargs['data'][0]['id']
            self.assertEqual(validated_id , invalid_cases[0])

    def test_saleitems_applied_validate_error(self):
        invalid_cases = [
            ('sale_item', None, 'This field may not be null.'),
            ('sale_item', '',   'This field may not be null.'),
            ('sale_item', -123, 'Invalid pk "-123" - object does not exist.'),
            ('sale_item',  999, 'Invalid pk "999" - object does not exist.'),
            ('sale_item', 'Gui','Incorrect type. Expected pk value, received str.'),
            ('unit',       None, 'This field may not be null.'),
            ('unit',        999, '"999" is not a valid choice.'),
            ('unit',       '+-+-', '"+-+-" is not a valid choice.'),
            ('quantity',   None, 'This field may not be null.'),
            ('quantity', -0.3,  NumberBoundaryValidator._error_msg_pattern % (-0.3, 0.0, 'gt')),
            ('quantity', -0.0,  NumberBoundaryValidator._error_msg_pattern % (-0.0, 0.0, 'gt')),
            ('quantity',  0.0,  NumberBoundaryValidator._error_msg_pattern % ( 0.0, 0.0, 'gt')),
        ]
        self.serializer_kwargs['data'] = list(filter(lambda d: any(d['saleitems_applied']), \
                self.serializer_kwargs['data']))
        for idx in range(len(self.serializer_kwargs['data'])):
            rand_chosen_idx_2 = random.randrange(0, len(self.serializer_kwargs['data'][idx]['saleitems_applied']))
            fn_choose_edit_item = lambda x : x[idx]['saleitems_applied'][rand_chosen_idx_2]
            self._loop_through_invalid_cases_common(fn_choose_edit_item, invalid_cases)


    def _loop_through_invalid_cases_common(self, fn_choose_edit_item, invalid_cases, **kwargs):
        serializer = self.serializer_class( **self.serializer_kwargs )
        req_data = fn_choose_edit_item( serializer.initial_data )
        for field_name, invalid_value, expect_err_msg in invalid_cases:
            self._assert_single_invalid_case(testcase=self, field_name=field_name, invalid_value=invalid_value,
                    expect_err_msg=expect_err_msg,  req_data=req_data, serializer=serializer,
                    fn_choose_edit_item=fn_choose_edit_item)
## end of class SaleablePkgCreationTestCase


class SaleablePkgBaseUpdateTestCase(SaleablePkgCommonMixin, TransactionTestCase):
    def setUp(self):
        super().setUp()
        # create new instances first
        serializer_kwargs_setup = {'data': self.request_data, 'many': True, 'usrprof_id': self.profile_ids[0],}
        serializer = self.serializer_class( **serializer_kwargs_setup )
        serializer.is_valid(raise_exception=True)
        self._created_items = serializer.save()
        self.request_data = list(map(dict, serializer.data))


class SaleablePkgUpdateTestCase(SaleablePkgBaseUpdateTestCase):
    min_num_pkgs  = 4
    min_num_applied_attrs = 2

    def setUp(self):
        super().setUp()
        num_edit_data = len(self.request_data) >> 1
        self.editing_data    = copy.deepcopy(self.request_data[:num_edit_data])
        self.unaffected_data = self.request_data[num_edit_data:]
        editing_ids = tuple(map(lambda x:x['id'], self.editing_data))
        self.edit_objs =  self.serializer_class.Meta.model.objects.filter(id__in=editing_ids)
    ## end of setUp()

    def test_bulk_ok_some_items(self):
        self.rand_gen_edit_data(editing_data=self.editing_data)
        serializer_kwargs = {'data': copy.deepcopy(self.editing_data),  'instance': self.edit_objs, 'many': True,}
        serializer = self.serializer_class( **serializer_kwargs )
        serializer.is_valid(raise_exception=True)
        edited_objs = serializer.save()
        self.verify_data(actual_data=edited_objs, expect_data=self.editing_data,
                usrprof_id=self.profile_ids[0], verify_id=True)
        unaffected_ids = tuple(map(lambda x:x['id'], self.unaffected_data))
        unaffected_objs = self.serializer_class.Meta.model.objects.filter(id__in=unaffected_ids).order_by('id')
        sorted_unaffected_data = sorted(self.unaffected_data, key=lambda d:d['id'])
        self.verify_data(actual_data=unaffected_objs, expect_data=sorted_unaffected_data,
                usrprof_id=self.profile_ids[0], verify_id=True)
## end of class class SaleablePkgUpdateTestCase


class SaleablePkgUpdateConflictTestCase(SaleablePkgBaseUpdateTestCase):
    min_num_pkgs  = 4
    min_num_applied_saleitems = 3

    def setUp(self):
        super().setUp()
        num_edit_data = len(self.request_data) >> 1
        self.editing_data    = copy.deepcopy(self.request_data[:num_edit_data])
        self.unaffected_data = self.request_data[num_edit_data:]
        editing_ids = tuple(map(lambda x:x['id'], self.editing_data))
        self.edit_objs =  self.serializer_class.Meta.model.objects.filter(id__in=editing_ids)


    def test_conflict_pkg_id(self):
        self.assertGreaterEqual(len(self.editing_data) , 2)
        discarded_id = self.editing_data[0]['id']
        self.editing_data[0]['id'] = self.editing_data[1]['id']
        edit_objs = self.edit_objs.exclude(pk=discarded_id)
        serializer_kwargs = {'data': self.editing_data, 'many': True, 'instance': edit_objs}
        error_caught = None
        with self.assertRaises(DRFValidationError):
            try:
                serializer = self.serializer_class( **serializer_kwargs )
                serializer.is_valid(raise_exception=True)
            except DRFValidationError as e:
                error_caught = e
                raise
        self.assertNotEqual(error_caught, None)
        non_field_err_key = drf_default_settings['NON_FIELD_ERRORS_KEY']
        err_detail = error_caught.detail[non_field_err_key][0]
        err_info = json.loads(str(err_detail))
        self.assertEqual(err_detail.code, 'conflict')
        self.assertEqual(err_info['message'], 'duplicate item found in the list')
        err_ids = [e['id'] for e in err_info['value']]
        self.assertNotIn(discarded_id, err_ids)
        self.assertEqual(err_info['value'][0], err_info['value'][1])


    def test_conflict_composite_id(self):
        for data in self.editing_data:
            self.assertGreaterEqual(len(data['saleitems_applied']) , 2)
        composite_data = self.editing_data[0]['saleitems_applied']
        discarded_id = composite_data[1]['sale_item']
        composite_data[1]['sale_item'] = composite_data[0]['sale_item']
        serializer_kwargs = {'data': copy.deepcopy(self.editing_data), 'many': True, 'instance': self.edit_objs}
        serializer = self.serializer_class( **serializer_kwargs )
        serializer.is_valid(raise_exception=True)
        validated_data = serializer.validated_data[0]['saleitems_applied']
        self.assertEqual(validated_data[0]['sale_item'] , validated_data[1]['sale_item'])
        self.assertEqual(validated_data[1]['sale_item'].id , composite_data[1]['sale_item'])
        for field_name in ['unit', 'quantity']:
            self.assertEqual(validated_data[0][field_name] , composite_data[0][field_name])
            self.assertEqual(validated_data[1][field_name] , composite_data[1][field_name])
        edited_objs = serializer.save()
        # the first composite instance should be lost
        discarded_composite = composite_data[0]
        self.editing_data[0]['saleitems_applied'] = composite_data[1:]
        self.verify_data(actual_data=edited_objs, expect_data=self.editing_data,
                usrprof_id=self.profile_ids[0], verify_id=True)
## end of class SaleablePkgUpdateConflictTestCase


class SaleablePkgRepresentationTestCase(SaleablePkgBaseUpdateTestCase):
    def test_represent_all(self):
        created_ids  = tuple(map(lambda x:x.id, self._created_items))
        created_objs = self.serializer_class.Meta.model.objects.filter(id__in=created_ids)
        serializer_ro = self.serializer_class(instance=created_objs, many=True)
        self.verify_data(actual_data=created_objs, expect_data=serializer_ro.data,
                usrprof_id=self.profile_ids[0], verify_id=True)

    def test_represent_partial(self):
        expect_fields = ['id', 'name', 'price', 'saleitems_applied']
        mocked_request = Mock()
        mocked_request.query_params = {'fields': ','.join(expect_fields)}
        context = {'request': mocked_request}
        created_ids  = tuple(map(lambda x:x.id, self._created_items))
        created_objs = self.serializer_class.Meta.model.objects.filter(id__in=created_ids)
        serializer_ro = self.serializer_class(instance=created_objs, many=True, context=context)

        expect_d_iter = iter( serializer_ro.data)
        for actual_d in created_objs:
            expect_d = next(expect_d_iter)
            bound_assert_fn = partial(assert_field_equal, testcase=self, actual_obj=actual_d, expect_obj=expect_d)
            tuple(map(bound_assert_fn, ['id', 'name', 'price',]))
            diff_composite(testcase=self, expect_d=expect_d['saleitems_applied'], actual_d=actual_d,
                    lower_elm_name='sale_item', lower_elm_mgr_field='saleitems_applied')

