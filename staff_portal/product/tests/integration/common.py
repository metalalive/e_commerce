import random
from functools import partial

from product.models.base import _ProductAttrValueDataType, ProductTagClosure
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
        {'id':25, 'name': 'Length of square (Ft.)', 'dtype': _ProductAttrValueDataType.FLOAT.value[0][0]},
        {'id':26, 'name': 'Diameter (In.)', 'dtype': _ProductAttrValueDataType.FLOAT.value[0][0]},
        {'id':27, 'name': 'max resistence voltage', 'dtype': _ProductAttrValueDataType.FLOAT.value[0][0]},
    ],
    'ProductAttributeValueStr': ['sticky', 'crunchy', 'silky', 'chewy', 'crispy', 'meaty', 'eggy'],
    'ProductAttributeValuePosInt': [5,2,35,42,9853,15,53,104,57,53,83],
    'ProductAttributeValueInt': [-10,-23,-38,-4,-85,-7,-887,-960,-20,-515,-61,-34],
    'ProductAttributeValueFloat': [1.2 , 3.45, 6.47, 8.901, 2.34, 50.6, 6.778, 9.01, 22.3],
    'ProductAppliedAttributePrice': [10.4 , 59, 80.3, 19.4, 94.2, 13.4, 5.67, 88.9],
    'ProductDevIngredient': [
        {'id':2, 'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'tomato'},
        {'id':3, 'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'all-purpose flour'},
        {'id':4, 'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'bread flour'},
        {'id':5, 'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'quail egg'},
        {'id':6, 'category':ProductDevIngredientType.RAW_MATERIAL    , 'name':'dry yeast powder'},
        {'id':7, 'category':ProductDevIngredientType.WORK_IN_PROGRESS, 'name':'poolish'},
        {'id':8, 'category':ProductDevIngredientType.WORK_IN_PROGRESS, 'name':'tomato puree'},
        {'id':9, 'category':ProductDevIngredientType.WORK_IN_PROGRESS, 'name':'LiPo Battery'},
        {'id':10, 'category':ProductDevIngredientType.WORK_IN_PROGRESS  , 'name':'RISC-V SoC'},
        {'id':11, 'category':ProductDevIngredientType.WORK_IN_PROGRESS  , 'name':'ARM Cortex-A72 SoC'},
        {'id':12, 'category':ProductDevIngredientType.WORK_IN_PROGRESS  , 'name':'Pixhawk flight controller'},
        {'id':13, 'category':ProductDevIngredientType.WORK_IN_PROGRESS  , 'name':'GPS sensor'},
        {'id':14, 'category':ProductDevIngredientType.CONSUMABLES     , 'name':'bio gas'},
        {'id':15, 'category':ProductDevIngredientType.EQUIPMENTS      , 'name':'oven'},
        {'id':16, 'category':ProductDevIngredientType.EQUIPMENTS  , 'name':'Soldering kit'},
        {'id':17, 'category':ProductDevIngredientType.EQUIPMENTS  , 'name':'Portable Oscilloscope'},
        {'id':18, 'category':ProductDevIngredientType.EQUIPMENTS  , 'name':'Logic Analyzer'},
    ],
    'ProductSaleableItemMedia': [
        {'media':'384gaeirj4jg393P'},
        {'media':'92u4t09u4tijq3oti'},
        {'media':'2903tijtg3h4teg'},
        {'media':'09fawgsdkmbiehob'},
        {'media':'2093jti4jt0394ut'},
        {'media':'0fwkbb0erwrw#rqrt'},
        {'media':'309ur204t42jWh1'},
        {'media':'eOy1r0j4SKuAYEre'},
    ],
    'ProductSaleableItem': [
        {'visible':  True, 'name':'Raspberry PI 4 dev board', 'price':3.88,  'usrprof':19},
        {'visible': False, 'name':'SiFive HiFive Unmatched' , 'price':11.30, 'usrprof':28},
        {'visible':  True, 'name':'rough rice noddle',   'price':0.18,  'usrprof':22},
        {'visible': False, 'name':'Mozzarella pizza', 'price':13.93, 'usrprof':79},
        {'visible':  True, 'name':'Pita dough', 'price':3.08, 'usrprof':79},
        {'visible': False, 'name':'quad drone', 'price':17.02, 'usrprof':12},
        {'visible':  True, 'name':'Industrial Fan', 'price':29.10, 'usrprof':3},
    ],
} # end of _fixtures


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
    nodes = list(map(_gen_closure_node, data))
    ProductTagClosure.objects.bulk_create(nodes)
    return nodes

