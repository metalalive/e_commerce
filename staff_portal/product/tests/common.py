import copy
import json
import random
from functools import partial

from django.db.utils import IntegrityError, DataError
from django.contrib.contenttypes.models  import ContentType

from common.util.python import flatten_nested_iterable
from product.models.base import _ProductAttrValueDataType
from product.models.development import ProductDevIngredientType

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
        {'id':25, 'name': 'min. dormant temperature (celsius)', 'dtype': _ProductAttrValueDataType.INTEGER.value[0][0]},
        {'id':26, 'name': 'Length of square (Ft.)', 'dtype': _ProductAttrValueDataType.FLOAT.value[0][0]},
        {'id':27, 'name': 'Diameter (In.)', 'dtype': _ProductAttrValueDataType.FLOAT.value[0][0]},
        {'id':28, 'name': 'max resistence voltage', 'dtype': _ProductAttrValueDataType.FLOAT.value[0][0]},
        {'id':29, 'name': 'min distance between 2 metal wires (um)', 'dtype': _ProductAttrValueDataType.FLOAT.value[0][0]},
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


def _null_test_obj_attrs(testcase, instance, field_names):
    for fname in field_names:
        old_value = getattr(instance, fname)
        setattr(instance, fname, None)
        with testcase.assertRaises(IntegrityError) as e:
            instance.save(force_insert=True)
        setattr(instance, fname, old_value)


def _gen_ingredient_attrvals(attrtype_ref, ingredient, idx, extra_charge=None):
    ingredient_ct = ContentType.objects.get_for_model(ingredient)
    model_cls = attrtype_ref.attr_val_set.model
    num_limit = len(_fixtures[model_cls.__name__])
    new_value = _fixtures[model_cls.__name__][idx % num_limit]
    model_init_kwargs = {
        'ingredient_type': ingredient_ct,  'ingredient_id':ingredient.pk,
        'attr_type':attrtype_ref, 'value':new_value
    }
    if extra_charge and extra_charge > 0.0:
        model_init_kwargs['extra_amount'] = extra_charge
    return model_cls(**model_init_kwargs)


def _ingredient_attrvals_common_setup(attrtypes_gen_fn, ingredients):
    _attrval_objs = {item[0][0]:{} for item in _ProductAttrValueDataType}
    idx = 0
    for ingredient in ingredients:
        attrtypes_gen = attrtypes_gen_fn()
        for attrtype_ref in attrtypes_gen:
            if _attrval_objs[attrtype_ref.dtype].get(ingredient.pk) is None:
                _attrval_objs[attrtype_ref.dtype][ingredient.pk] = []
            attrval = _gen_ingredient_attrvals(attrtype_ref, ingredient, idx)
            _attrval_objs[attrtype_ref.dtype][ingredient.pk].append(attrval)
            idx += 1
    for dtype, objmap in _attrval_objs.items():
        related_field_name = _ProductAttrValueDataType.related_field_map(dtype_code=dtype)
        related_field_mgr = getattr(ingredients[0], related_field_name)
        objs = tuple(flatten_nested_iterable(list_=[x for x in objmap.values()]))
        related_field_mgr.bulk_create(objs)
    return _attrval_objs


class HttpRequestDataGen:
    def customize_req_data_item(self, item, **kwargs):
        raise NotImplementedError()

