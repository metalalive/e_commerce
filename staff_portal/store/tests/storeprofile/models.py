import random
import string
import pytest

from store.models import StoreProfile, StoreEmail, StorePhone, OutletLocation, HourOfOperation
from store.tests.common import db_engine_resource, session_for_test, session_for_setup, store_data, email_data, phone_data, loc_data, opendays_data

# module-level test setup / teardown
def setup_module(module):
    pass

def teardown_module(module):
    pass


def _saved_obj_gen(store_data_gen, email_data_gen, phone_data_gen, session):
    num_emails_per_store = 2
    num_phones_per_store = 3
    num_staff_per_store = 4
    num_products_per_store = 5
    while True:
        new_item = next(store_data_gen)
        new_item['emails'] = [StoreEmail(**next(email_data_gen)) for _ in range(num_emails_per_store)]
        new_item['phones'] = [StorePhone(**next(phone_data_gen)) for _ in range(num_phones_per_store)]
        obj = StoreProfile(**new_item)
        StoreProfile.bulk_insert([obj], session=session)
        yield obj

@pytest.fixture
def saved_store_objs(store_data, email_data, phone_data, session_for_setup):
    return _saved_obj_gen(store_data, email_data_gen=email_data, phone_data_gen=phone_data,
            session=session_for_setup)



class TestCreation: # class name must start with TestXxxx
    def test_bulk_ok(self, session_for_test, store_data):
        instantiate_fn = lambda d: StoreProfile(**d)
        _store_data = map(lambda idx: next(store_data), range(6))
        objs = list(map(instantiate_fn, _store_data))
        target_ids = [obj.supervisor_id for obj in objs]
        chk_result1 = StoreProfile.quota_stats(objs, session=session_for_test, target_ids=target_ids)
        for supervisor_id, usage in chk_result1.items():
            assert usage['num_new_items'] == 1
            assert usage['num_existing_items'] == 0
        session_for_test.add_all(objs)
        session_for_test.add_all(objs)
        StoreProfile.bulk_insert(objs, session=session_for_test)
        _fn_get_pk = lambda obj:obj.id
        ids = tuple(map(_fn_get_pk, objs))
        query = session_for_test.query(StoreProfile).filter(StoreProfile.id.in_(ids))
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
        chk_result2 = StoreProfile.quota_stats([], session=session_for_test, target_ids=target_ids)
        for supervisor_id, usage in chk_result2.items():
            assert usage['num_new_items'] == 0
            assert usage['num_existing_items'] == 1

    def test_duplicate_ids_1(self, session_for_test, store_data):
        instantiate_fn = lambda d: StoreProfile(**d)
        _store_data = map(lambda idx: next(store_data), range(8))
        objs = list(map(instantiate_fn, _store_data))
        dup_ids = [1234, 5678, 9012]
        objs[0].id = dup_ids[0]
        objs[1].id = dup_ids[1]
        objs[2].id = dup_ids[2]
        StoreProfile.bulk_insert(objs[:3], session=session_for_test)
        objs[3].id = dup_ids[0]
        objs[4].id = dup_ids[1]
        objs[5].id = dup_ids[2]
        StoreProfile.bulk_insert(objs[3:], session=session_for_test)
        ids = map(lambda obj: obj.id, objs)
        ids = tuple(filter(lambda x: x is not None and x > 0, ids))
        expect_value = len(ids)
        actual_value = len(set(ids)) # test whether all ID numbers are distinct to each other
        assert actual_value > 0
        assert expect_value == actual_value
        assert objs[3].id != dup_ids[0]
        assert objs[4].id != dup_ids[1]
        assert objs[5].id != dup_ids[2]

    def test_duplicate_ids_2(self, session_for_test, store_data):
        instantiate_fn = lambda d: StoreProfile(**d)
        _store_data = map(lambda idx: next(store_data), range(7))
        objs = list(map(instantiate_fn, _store_data))
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
                StoreProfile.bulk_insert(objs, session=session_for_test)
            except ValueError as e:
                error_caught = e
                raise
        assert error_caught.args[0] == 'Detect duplicate IDs from application caller'


    def test_quota_statistics(self, session_for_test, store_data):
        supervisor_id = 91
        target_ids = [supervisor_id]
        def instantiate_fn(d):
            d['supervisor_id'] = supervisor_id
            return  StoreProfile(**d)
        _store_data = map(lambda idx: next(store_data), range(8))
        objs = list(map(instantiate_fn, _store_data))
        parts = [objs[0:2], objs[2:5], objs[5:]]
        StoreProfile.bulk_insert(parts[0], session=session_for_test)
        chk_result = StoreProfile.quota_stats(parts[1], session=session_for_test, target_ids=target_ids)
        assert chk_result[supervisor_id]['num_existing_items'] == len(parts[0])
        assert chk_result[supervisor_id]['num_new_items'] == len(parts[1])
        StoreProfile.bulk_insert(parts[1], session=session_for_test)
        chk_result = StoreProfile.quota_stats(parts[2], session=session_for_test, target_ids=target_ids)
        assert chk_result[supervisor_id]['num_existing_items'] == len(parts[0]) + len(parts[1])
        assert chk_result[supervisor_id]['num_new_items'] == len(parts[2])


    def test_bulk_with_related_fields(self, session_for_test, store_data, email_data, phone_data,
            loc_data, opendays_data):
        num_stores = 3
        num_emails_per_store = 2
        num_phones_per_store = 3
        num_opendays_per_store = 4

        def instantiate_fn(d):
            email_data_gen = map(lambda idx: next(email_data), range(num_emails_per_store))
            phone_data_gen = map(lambda idx: next(phone_data), range(num_phones_per_store))
            opendays_gen = map(lambda idx: next(opendays_data), range(num_opendays_per_store))
            d['emails'] = list(map(lambda d:StoreEmail(**d), email_data_gen))
            d['phones'] = list(map(lambda d:StorePhone(**d), phone_data_gen))
            d['location'] = OutletLocation(**next(loc_data))
            d['open_days'] = list(map(lambda d: HourOfOperation(**d), opendays_gen))
            return  StoreProfile(**d)

        _store_data = map(lambda idx: next(store_data), range(num_stores))
        objs = list(map(instantiate_fn, _store_data))
        StoreProfile.bulk_insert(objs, session=session_for_test)
        _fn_get_pk = lambda obj:obj.id
        ids = tuple(map(_fn_get_pk, objs))
        query = session_for_test.query(StoreProfile).filter(StoreProfile.id.in_(ids))
        objs = query.all()
        chk_result = StoreEmail.quota_stats([], session=session_for_test, target_ids=ids)
        for store_id, info in chk_result.items():
            assert info['num_existing_items'] == num_emails_per_store
        chk_result = StorePhone.quota_stats([], session=session_for_test, target_ids=ids)
        for store_id, info in chk_result.items():
            assert info['num_existing_items'] == num_phones_per_store
## end of TestCreation


class TestUpdate:
    def test_edit_contact(self, session_for_test, email_data, phone_data, saved_store_objs):
        import pdb
        pdb.set_trace()
        pass

    def test_edit_staff(self):
        pass

    def test_edit_product(self):
        pass


