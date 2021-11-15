import random
import string

import pytest

from store.models import StoreProfile
from store.tests.common import db_engine_resource, db_session

@pytest.fixture
def store_data():
    return [{'active':random.choice([True,False]), 'label':'agt',
        'supervisor_id':idx, 'id':None} for idx in range(2, 9)]

# module-level test setup / teardown
def setup_module(module):
    pass

def teardown_module(module):
    pass


class TestCreation: # class name must start with TestXxxx

    def test_bulk_ok(self, db_session, store_data):
        instantiate_fn = lambda d: StoreProfile(**d)
        objs = list(map(instantiate_fn, store_data))
        limit = {d['supervisor_id']: random.randrange(5,10) for d in store_data}
        chk_result1 = StoreProfile.quota_stats(objs, session=db_session, limit=limit, attname='supervisor_id')
        for supervisor_id, usage in chk_result1.items():
            assert usage['num_new_items'] == 1
            assert usage['num_existing_items'] == 0
            assert usage['max_items_limit'] >= usage['num_new_items'] + usage['num_existing_items']
        db_session.add_all(objs)
        db_session.add_all(objs)
        StoreProfile.bulk_save(objs, session=db_session)
        _fn_get_pk = lambda obj:obj.id
        ids = tuple(map(_fn_get_pk, objs))
        query = db_session.query(StoreProfile).filter(StoreProfile.id.in_(ids))
        expect_objs = sorted(objs, key=_fn_get_pk)
        actual_objs = query.all()
        actual_objs = sorted(actual_objs, key=_fn_get_pk)
        assert len(expect_objs) == len(actual_objs)
        expect_objs_iter = iter(expect_objs)
        for actual_obj in actual_objs:
            expect_obj = next(expect_objs_iter)
            assert actual_obj.id > 0
            assert actual_obj.id == expect_obj.id
            assert actual_obj is expect_obj
        chk_result2 = StoreProfile.quota_stats([], session=db_session, limit=limit, attname='supervisor_id')
        for supervisor_id, usage in chk_result2.items():
            assert usage['num_new_items'] == 0
            assert usage['num_existing_items'] == 1
            assert usage['max_items_limit'] >= usage['num_new_items'] + usage['num_existing_items']

    def test_duplicate_ids_1(self, db_session, store_data):
        instantiate_fn = lambda d: StoreProfile(**d)
        objs = list(map(instantiate_fn, store_data))
        dup_ids = [1234, 5678, 9012]
        objs[0].id = dup_ids[0]
        objs[1].id = dup_ids[1]
        objs[2].id = dup_ids[2]
        StoreProfile.bulk_save(objs[:3], session=db_session)
        objs[3].id = dup_ids[0]
        objs[4].id = dup_ids[1]
        objs[5].id = dup_ids[2]
        StoreProfile.bulk_save(objs[3:], session=db_session)
        ids = map(lambda obj: obj.id, objs)
        ids = tuple(filter(lambda x: x is not None and x > 0, ids))
        expect_value = len(ids)
        actual_value = len(set(ids)) # test whether all ID numbers are distinct to each other
        assert actual_value > 0
        assert expect_value == actual_value
        assert objs[3].id != dup_ids[0]
        assert objs[4].id != dup_ids[1]
        assert objs[5].id != dup_ids[2]

    def test_duplicate_ids_2(self, db_session, store_data):
        instantiate_fn = lambda d: StoreProfile(**d)
        objs = list(map(instantiate_fn, store_data))
        dup_ids = [1234, 5678, 9012]
        objs[0].id = dup_ids[0]
        objs[1].id = dup_ids[1]
        objs[2].id = dup_ids[2]
        objs[3].id = dup_ids[0]
        objs[4].id = dup_ids[1]
        objs[5].id = dup_ids[2]
        error_caught = None
        with pytest.raises(ValueError):
            try:
                StoreProfile.bulk_save(objs, session=db_session)
            except ValueError as e:
                error_caught = e
                raise
        assert error_caught.args[0] == 'Detect duplicate IDs from application caller'


    def test_invalid_input(self):
        pass

    def test_quota_statistics(self):
        pass

