import copy
import json
import random
from functools import partial

from django.db.models.constants import LOOKUP_SEP

from common.models.enums   import UnitOfMeasurement
from common.util.python  import import_module_string
from product.models.base import ProductTag, ProductTagClosure, ProductAttributeType, _ProductAttrValueDataType, ProductSaleableItem
from product.models.development import ProductDevIngredientType, ProductDevIngredient
from product.serializers.base import SaleableItemSerializer

_fixtures = {
    'AuthUser': [
        {'id':14, 'is_staff':True,  'is_active':True,  'username': 'yusir001','password': '93rutGrPt'} ,
        {'id':19, 'is_staff':False, 'is_active':True,  'username': 'yusir002','password': '39rjfrret'} ,
        {'id':10, 'is_staff':True,  'is_active':False, 'username': 'yusir003','password': 'if74w#gfy'} ,
    ],
    'ProductTag': [
        {'id':30 , 'usrprof': 56,'name':'Food & Beverage'}               ,
        {'id':31 , 'usrprof': 56,'name':'DIY hardware'}                  ,
        {'id':32 , 'usrprof': 56,'name':'Dairy'}                         ,
        {'id':33 , 'usrprof': 56,'name':'Farm Produce'}                  ,
        {'id':34 , 'usrprof': 56,'name':'Embedded system device'}        ,
        {'id':35 , 'usrprof': 56,'name':'semi-prepared food ingredient'} ,
        {'id':36 , 'usrprof': 56,'name':'Veggie'}                        ,
        {'id':37 , 'usrprof': 56,'name':'Fruit'}                         ,
        {'id':38 , 'usrprof': 56,'name':'Debugging device'}              ,
    ],
    'ProductTagClosure': [
        {'id':1, 'ancestor':30, 'descendant':30, 'depth':0},
        {'id':2, 'ancestor':31, 'descendant':31, 'depth':0},
        {'id':3, 'ancestor':32, 'descendant':32, 'depth':0},
        {'id':4, 'ancestor':33, 'descendant':33, 'depth':0},
        {'id':5, 'ancestor':34, 'descendant':34, 'depth':0},
        {'id':6, 'ancestor':35, 'descendant':35, 'depth':0},
        {'id':7, 'ancestor':36, 'descendant':36, 'depth':0},
        {'id':8, 'ancestor':37, 'descendant':37, 'depth':0},
        {'id':9, 'ancestor':30, 'descendant':32, 'depth':1},
        {'id':10, 'ancestor':30, 'descendant':33, 'depth':1},
        {'id':11, 'ancestor':30, 'descendant':35, 'depth':1},
        {'id':12, 'ancestor':33, 'descendant':36, 'depth':1},
        {'id':13, 'ancestor':33, 'descendant':37, 'depth':1},
        {'id':14, 'ancestor':30, 'descendant':36, 'depth':2},
        {'id':15, 'ancestor':30, 'descendant':37, 'depth':2},
        {'id':16, 'ancestor':31, 'descendant':34, 'depth':1},
        {'id':17, 'ancestor':38, 'descendant':38, 'depth':0},
        {'id':18, 'ancestor':31, 'descendant':38, 'depth':1},
    ],
    'ProductAttributeType': [
        {'id':20, 'name': 'toppings category', 'dtype': _ProductAttrValueDataType.STRING.value[0][0]},
        {'id':21, 'name': 'color', 'dtype': _ProductAttrValueDataType.STRING.value[0][0]},
        {'id':22, 'name': 'bread crust level', 'dtype': _ProductAttrValueDataType.POSITIVE_INTEGER.value[0][0]},
        {'id':23, 'name': 'cache size (KBytes)', 'dtype': _ProductAttrValueDataType.POSITIVE_INTEGER.value[0][0]},
        {'id':24, 'name': 'min. working temperature (celsius)', 'dtype': _ProductAttrValueDataType.INTEGER.value[0][0]},
        {'id':25, 'name': 'Length of square (Ft.)', 'dtype': _ProductAttrValueDataType.FLOAT.value[0][0]},
        {'id':26, 'name': 'Diameter (In.)', 'dtype': _ProductAttrValueDataType.FLOAT.value[0][0]},
        {'id':27, 'name': 'max resistence voltage', 'dtype': _ProductAttrValueDataType.FLOAT.value[0][0]},
        {'id':28, 'name': 'min distance between 2 metal wires (um)', 'dtype': _ProductAttrValueDataType.FLOAT.value[0][0]},
    ],
    'ProductAttributeValueStr': ['sticky', 'crunchy', 'chubby', 'chewy', 'crispy', 'meaty', 'creepy'],
    'ProductAttributeValuePosInt': [random.randrange(1,10000) for _ in range(25)],
    'ProductAttributeValueInt': [random.randrange(-10000,10000) for _ in range(25)],
    'ProductAttributeValueFloat': [random.randrange(-100,100) * 0.31415926 for _ in range(25)],
    'ProductAppliedAttributePrice': [10.4 , 59, 80.3, 19.4, 94.2, 13.4, 5.67, 88.9],
    'ProductDevIngredient': [
        {'id':2, 'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'tomato'},
        {'id':3, 'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'all-purpose flour'},
        {'id':4, 'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'bread flour'},
        {'id':5, 'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'quail egg'},
        {'id':6, 'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'dry yeast powder'},
        {'id':7, 'category':ProductDevIngredientType.WORK_IN_PROGRESS, 'name':'poolish'},
        {'id':8, 'category':ProductDevIngredientType.WORK_IN_PROGRESS, 'name':'tomato puree'},
        {'id':9, 'category':ProductDevIngredientType.WORK_IN_PROGRESS, 'name':'beef bone broth'},
        {'id':10, 'category':ProductDevIngredientType.WORK_IN_PROGRESS, 'name':'LiPo Battery'},
        {'id':11, 'category':ProductDevIngredientType.WORK_IN_PROGRESS  , 'name':'RISC-V SoC'},
        {'id':12, 'category':ProductDevIngredientType.WORK_IN_PROGRESS  , 'name':'ARM Cortex-A72 SoC'},
        {'id':13, 'category':ProductDevIngredientType.WORK_IN_PROGRESS  , 'name':'Pixhawk flight controller'},
        {'id':14, 'category':ProductDevIngredientType.WORK_IN_PROGRESS  , 'name':'GPS sensor'},
        {'id':15, 'category':ProductDevIngredientType.WORK_IN_PROGRESS  , 'name':'USB chip driver'},
        {'id':16, 'category':ProductDevIngredientType.CONSUMABLES     , 'name':'bio gas'},
        {'id':17, 'category':ProductDevIngredientType.EQUIPMENTS      , 'name':'oven'},
        {'id':18, 'category':ProductDevIngredientType.EQUIPMENTS  , 'name':'Soldering kit'},
        {'id':19, 'category':ProductDevIngredientType.EQUIPMENTS  , 'name':'Portable Oscilloscope'},
        {'id':20, 'category':ProductDevIngredientType.EQUIPMENTS  , 'name':'Logic Analyzer'},
    ],
    'ProductSaleableItemMedia': [
        {'media':'384gaeirj4jg393P'},
        {'media':'92u4t09u4tijq3oti'},
        {'media':'2903tijtg3h4teg'},
        {'media':'09fawgsdkmbiehob'},
        {'media':'2093jti4jt0394ut'},
        {'media':'0fwkbb0erwrwXrqrt'},
        {'media':'309ur204t42jWh1'},
        {'media':'eOy1r0j4SKuAYEre'},
    ],
    'ProductSaleableItem': [
        {'visible':  True, 'name':'Raspberry PI 4 Dev board', 'price':3.88,  'usrprof':19},
        {'visible': False, 'name':'SiFive HiFive Unmatched' , 'price':11.30, 'usrprof':212},
        {'visible':  True, 'name':'rough rice noOdle',   'price':0.18,  'usrprof':212},
        {'visible': False, 'name':'Mozzarella pizza', 'price':13.93, 'usrprof':79},
        {'visible':  True, 'name':'Pita dough', 'price':3.08, 'usrprof':79},
        {'visible': False, 'name':'quad drone', 'price':17.02, 'usrprof':212},
        {'visible':  True, 'name':'Industrial Fan', 'price':29.10, 'usrprof':53},
        {'visible': False, 'name':'Trail runner shoes', 'price': 69.9, 'usrprof':79},
        {'visible':  True, 'name':'concrete soil', 'price': 5.6, 'usrprof':28},
        {'visible': False, 'name':'Banana PI dev board', 'price':1.04,  'usrprof':53},
        {'visible':  True, 'name':'Semi-prepared Beef Noodle Soup',  'price':6.49,  'usrprof':53},
    ],
} # end of _fixtures


http_request_body_template = {
    'ProductSaleableItem': {
        'name': None,  'id': None, 'visible': None, 'price': None,
        'tags':[] ,
        'media_set':[],
        'attributes':[
            #{'id':None, 'type':None, 'value': None, 'extra_amount':None},
        ],
        'ingredients_applied': [
            #{'ingredient': None, 'unit': None, 'quantity': None},
        ]
    } # end of ProductSaleableItem
} # end of http_request_body_template


_load_init_params = lambda init_params, model_cls: model_cls(**init_params)

_modelobj_list_to_map = lambda list_: {item.pk: item for item in list_}

_dict_key_replace = lambda obj, from_, to_: {to_ if k == from_ else k: v for k,v in obj.items()}

_dict_kv_pair_evict = lambda obj, cond_fn: dict(filter(cond_fn, obj.items()))


def listitem_rand_assigner(list_, min_num_chosen:int=2, max_num_chosen:int=-1, distinct:bool=True):
    # utility for testing
    assert any(list_), 'input list should not be empty'
    assert min_num_chosen >= 0, 'min_num_chosen = %s' % min_num_chosen
    num_avail = len(list_)
    if max_num_chosen > 0:
        err_msg = 'max_num_chosen = %s, min_num_chosen = %s' % (max_num_chosen, min_num_chosen)
        assert max_num_chosen > min_num_chosen, err_msg
        if max_num_chosen > (num_avail + 1) and distinct is True:
            err_msg = 'num_avail = %s, max_num_chosen = %s, distinct = %s' \
                    % (num_avail, max_num_chosen, distinct)
            raise ValueError(err_msg)
    else:
        err_msg =  'num_avail = %s , min_num_chosen = %s' % (num_avail, min_num_chosen)
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


def _common_instances_setup(out:dict, models_info):
    """ create instances of given model classes in Django ORM """
    for model_cls, num_instance_required in models_info:
        bound_fn = partial(_load_init_params, model_cls=model_cls)
        model_name = model_cls.__name__
        ##params = _fixtures[model_name][:num_instance_required]
        params_gen = listitem_rand_assigner(list_=_fixtures[model_name],
                min_num_chosen=num_instance_required,
                max_num_chosen=(num_instance_required + 1))
        objs = list(map(bound_fn, params_gen))
        model_cls.objects.bulk_create(objs)
        out[model_name] = list(model_cls.objects.all())


def _product_tag_closure_setup(tag_map, data):
    _gen_closure_node = lambda d :ProductTagClosure(
            id    = d['id'],  depth = d['depth'],
            ancestor   = tag_map[d['ancestor']]  ,
            descendant = tag_map[d['descendant']]
        )
    filtered_data = filter(lambda d: tag_map.get(d['ancestor']) , data)
    nodes = list(map(_gen_closure_node, filtered_data))
    ProductTagClosure.objects.bulk_create(nodes)
    return nodes


def _saleitem_related_instance_setup(stored_models, num_tags=None, num_attrtypes=None, num_ingredients=None):
    model_fixtures = _fixtures
    if num_tags is None:
        num_tags = len(model_fixtures['ProductTag'])
    if num_attrtypes is None:
        num_attrtypes = len(model_fixtures['ProductAttributeType'])
    if num_ingredients is None:
        num_ingredients = len(model_fixtures['ProductDevIngredient'])
    models_info = [
            (ProductTag, num_tags),
            (ProductAttributeType, num_attrtypes  ),
            (ProductDevIngredient, num_ingredients),
        ]
    _common_instances_setup(out=stored_models, models_info=models_info)
    tag_map = _modelobj_list_to_map(stored_models['ProductTag'])
    stored_models['ProductTagClosure'] = _product_tag_closure_setup(
        tag_map=tag_map, data=model_fixtures['ProductTagClosure'])


def assert_softdelete_items_exist(testcase, deleted_ids, remain_ids, model_cls_path, id_label='id'):
    model_cls = import_module_string(dotted_path=model_cls_path)
    changeset = model_cls.SOFTDELETE_CHANGESET_MODEL
    cset = changeset.objects.filter(object_id__in=deleted_ids)
    testcase.assertEqual(cset.count(), len(deleted_ids))
    all_ids = []
    all_ids.extend(deleted_ids)
    all_ids.extend(remain_ids)
    query_id_key = LOOKUP_SEP.join([id_label, 'in'])
    lookup_kwargs = {'with_deleted':True, query_id_key: all_ids}
    qset = model_cls.objects.filter(**lookup_kwargs)
    testcase.assertEqual(qset.count(), len(all_ids))
    lookup_kwargs.pop('with_deleted')
    qset = model_cls.objects.filter(**lookup_kwargs)
    testcase.assertEqual(qset.count(), len(remain_ids))
    testcase.assertSetEqual(set(qset.values_list(id_label, flat=True)), set(remain_ids))
    qset = model_cls.objects.get_deleted_set()
    testcase.assertSetEqual(set(deleted_ids) , set(qset.values_list(id_label, flat=True)))


def rand_gen_request_body(template, customize_item_fn, data_gen):
    def rand_gen_single_req(data):
        single_req_item = copy.deepcopy(template)
        single_req_item.update(data)
        customize_item_fn(single_req_item)
        return single_req_item
    return map(rand_gen_single_req, data_gen)


def _get_inst_attr(obj, attname):
    if isinstance(obj, dict):
        out = obj[attname]
    else:
        out = getattr(obj, attname)
    return out


def assert_field_equal(fname, testcase, expect_obj, actual_obj):
    expect_val = _get_inst_attr(expect_obj,fname)
    actual_val = _get_inst_attr(actual_obj,fname)
    testcase.assertEqual(expect_val, actual_val)


class HttpRequestDataGen:
    def customize_req_data_item(self, item, **kwargs):
        raise NotImplementedError()


class HttpRequestDataGenSaleableItem(HttpRequestDataGen):
    min_num_applied_tags = 0
    min_num_applied_media = 0
    min_num_applied_attrs = 0
    min_num_applied_ingredients = 0

    def customize_req_data_item(self, item):
        model_fixtures = _fixtures
        applied_tag = listitem_rand_assigner(list_=model_fixtures['ProductTag'],
                min_num_chosen=self.min_num_applied_tags)
        applied_tag = map(lambda item:item['id'], applied_tag)
        item['tags'].extend(applied_tag)
        applied_media = listitem_rand_assigner(list_=model_fixtures['ProductSaleableItemMedia'],
                min_num_chosen=self.min_num_applied_media)
        applied_media = map(lambda m: m['media'], applied_media)
        item['media_set'].extend(applied_media)
        num_attrvals    = random.randrange(self.min_num_applied_attrs, len(model_fixtures['ProductAttributeType']))
        attr_dtypes_gen = listitem_rand_assigner(list_=model_fixtures['ProductAttributeType'],
                min_num_chosen=num_attrvals, max_num_chosen=(num_attrvals + 1))
        bound_gen_attr_val = partial(self._gen_attr_val, extra_amount_enabled=True)
        item['attributes'] = list(map(bound_gen_attr_val, attr_dtypes_gen))
        num_ingredients = random.randrange(self.min_num_applied_ingredients,
                len(model_fixtures['ProductDevIngredient']))
        ingredient_composite_gen = listitem_rand_assigner(list_=model_fixtures['ProductDevIngredient'],
                min_num_chosen=num_ingredients, max_num_chosen=(num_ingredients + 1))
        item['ingredients_applied'] = list(map(self._gen_ingredient_composite, ingredient_composite_gen))
    ## end of customize_req_data_item()


    def _gen_attr_val(self, attrtype, extra_amount_enabled):
        model_fixtures = _fixtures
        nested_item = {'id':None, 'type':_get_inst_attr(attrtype,'id'), 'value': None,}
        _fn = lambda option: option.value[0][0] == _get_inst_attr(attrtype,'dtype')
        dtype_option = filter(_fn, _ProductAttrValueDataType)
        dtype_option = tuple(dtype_option)[0]
        field_name = dtype_option.value[0][1]
        field_descriptor = getattr(ProductSaleableItem, field_name)
        attrval_cls_name = field_descriptor.field.related_model.__name__
        value_list = model_fixtures[attrval_cls_name]
        chosen_idx = random.randrange(0, len(value_list))
        nested_item['value'] = value_list[chosen_idx]
        rand_enable_extra_amount = random.randrange(0, 2)
        if extra_amount_enabled and rand_enable_extra_amount > 0:
            extra_amount_list = model_fixtures['ProductAppliedAttributePrice']
            chosen_idx = random.randrange(0, len(extra_amount_list))
            nested_item['extra_amount'] = float(extra_amount_list[chosen_idx])
        return nested_item

    def _gen_ingredient_composite(self, ingredient):
        chosen_idx = random.randrange(0, len(UnitOfMeasurement.choices))
        chosen_unit = UnitOfMeasurement.choices[chosen_idx][0]
        return {'ingredient': _get_inst_attr(ingredient,'id'), 'unit': chosen_unit,
                'quantity': float(random.randrange(1,25))}

## end of class HttpRequestDataGenSaleableItem

class BaseVerificationMixin:
    serializer_class = None
    def verify_objects(self, actual_instances, expect_data,  **kwargs):
        raise NotImplementedError()

    def verify_data(self, actual_data, expect_data, **kwargs):
        raise NotImplementedError()


class SaleableItemVerificationMixin(BaseVerificationMixin):
    serializer_class = SaleableItemSerializer

    def _assert_simple_fields(self, check_fields,  exp_sale_item, ac_sale_item, usrprof_id=None):
        self.assertNotEqual(_get_inst_attr(ac_sale_item,'id'), None)
        self.assertGreater(_get_inst_attr(ac_sale_item,'id'), 0)
        bound_assert_fn = partial(assert_field_equal, testcase=self,  expect_obj=exp_sale_item, actual_obj=ac_sale_item)
        tuple(map(bound_assert_fn, check_fields))
        if usrprof_id:
            self.assertEqual(_get_inst_attr(ac_sale_item,'usrprof'), usrprof_id)

    def _assert_product_attribute_fields(self, exp_sale_item, ac_sale_item):
        key_evict_condition = lambda kv: (kv[0] not in ('id', 'ingredient_type', 'ingredient_id')) \
                and not (kv[0] == 'extra_amount' and kv[1] is None)
        bound_dict_kv_pair_evict = partial(_dict_kv_pair_evict,  cond_fn=key_evict_condition)
        bound_dict_key_replace = partial(_dict_key_replace, to_='extra_amount', from_='_extra_charge__amount')
        for dtype_option in _ProductAttrValueDataType:
            field_name = dtype_option.value[0][1]
            expect_vals = exp_sale_item.get(field_name, None)
            if not expect_vals:
                continue
            expect_vals = list(map(bound_dict_kv_pair_evict, expect_vals))
            manager = _get_inst_attr(ac_sale_item, field_name)
            actual_vals = manager.values('attr_type', 'value', '_extra_charge__amount')
            actual_vals = map(bound_dict_key_replace, actual_vals)
            actual_vals = list(map(bound_dict_kv_pair_evict, actual_vals))
            expect_vals = sorted(expect_vals, key=lambda x:x['attr_type'])
            actual_vals = sorted(actual_vals, key=lambda x:x['attr_type'])
            expect_vals = json.dumps(expect_vals, sort_keys=True)
            actual_vals = json.dumps(actual_vals, sort_keys=True)
            self.assertEqual(expect_vals, actual_vals)

    def _assert_tag_fields(self, exp_sale_item, ac_sale_item):
        expect_vals = exp_sale_item['tags']
        if isinstance(ac_sale_item, dict):
            actual_vals = ac_sale_item['tags']
        else:
            actual_vals = list(ac_sale_item.tags.values_list('pk', flat=True))
        self.assertSetEqual(set(expect_vals), set(actual_vals))

    def _assert_mediaset_fields(self, exp_sale_item, ac_sale_item):
        expect_vals = exp_sale_item['media_set']
        if isinstance(ac_sale_item, dict):
            actual_vals = ac_sale_item['media_set']
        else:
            actual_vals = list(ac_sale_item.media_set.values_list('media', flat=True))
        self.assertSetEqual(set(expect_vals), set(actual_vals))

    def _assert_ingredients_applied_fields(self, exp_sale_item, ac_sale_item):
        sort_key_fn = lambda x:x['ingredient']
        expect_vals = exp_sale_item['ingredients_applied']
        if isinstance(ac_sale_item, dict):
            actual_vals = list(map(lambda d: dict(d), ac_sale_item['ingredients_applied']))
            tuple(map(lambda d: d.pop('sale_item', None), actual_vals))
        else:
            actual_vals = list(ac_sale_item.ingredients_applied.values('ingredient','unit','quantity'))
        expect_vals = sorted(expect_vals, key=sort_key_fn)
        actual_vals = sorted(actual_vals, key=sort_key_fn)
        self.assertListEqual(expect_vals, actual_vals)

    def _get_non_nested_fields(self, skip_id=True, skip_usrprof=True):
        check_fields = copy.copy(self.serializer_class.Meta.fields)
        if skip_id:
            check_fields.remove('id')
        if skip_usrprof:
            check_fields.remove('usrprof')
        return check_fields

    def verify_objects(self, actual_instances, expect_data, usrprof_id=None):
        non_nested_fields = self._get_non_nested_fields()
        expect_data = iter(expect_data)
        for ac_sale_item in actual_instances:
            exp_sale_item = next(expect_data)
            self._assert_simple_fields(non_nested_fields, exp_sale_item, ac_sale_item, usrprof_id)
            self._assert_tag_fields(exp_sale_item, ac_sale_item)
            self._assert_mediaset_fields(exp_sale_item, ac_sale_item)
            self._assert_ingredients_applied_fields(exp_sale_item, ac_sale_item)
            self._assert_product_attribute_fields(exp_sale_item, ac_sale_item)
    ## end of  def verify_objects()


    def verify_data(self, actual_data, expect_data, usrprof_id=None):
        non_nested_fields = self._get_non_nested_fields()
        expect_data = iter(expect_data)
        for ac_sale_item in actual_data:
            exp_sale_item = next(expect_data)
            self._assert_simple_fields(non_nested_fields, exp_sale_item, ac_sale_item, usrprof_id)
            self._assert_tag_fields(exp_sale_item, ac_sale_item)
            self._assert_mediaset_fields(exp_sale_item, ac_sale_item)
            self._assert_ingredients_applied_fields(exp_sale_item, ac_sale_item)
            self._assert_product_attribute_fields(exp_sale_item, ac_sale_item)
    ## end of  def verify_data()
## end of class SaleableItemVerificationMixin


