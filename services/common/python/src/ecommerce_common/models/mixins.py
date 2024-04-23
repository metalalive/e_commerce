import random

from ecommerce_common.models.db import get_sql_table_pk_gap_ranges


class MinimumInfoMixin:
    """
    callers can simply run the function minimum_info to retrieve minimal information
    without knowing exact field names for representation purpose.
    Subclasses can add few more the field values by overriding this function.
    """

    @property
    def minimum_info(self):
        if not hasattr(self, "min_info_field_names"):
            raise NotImplementedError
        field_names = getattr(self, "min_info_field_names")
        return {fname: getattr(self, fname) for fname in field_names}


class IdGapNumberFinder:
    MAX_GAP_VALUE = pow(2, 32) - 1
    _finder_orm_map = {}

    def __new__(cls, orm_model_class, *args, **kwargs):
        pkg_path = "%s.%s" % (orm_model_class.__module__, orm_model_class.__name__)
        instance = cls._finder_orm_map.get(pkg_path, None)
        if not instance:
            instance = super().__new__(cls, *args, **kwargs)
            instance.orm_model_class = orm_model_class
            cls._finder_orm_map[pkg_path] = instance
        return instance

    def _assert_any_dup_id(self, instances, id_field_name="id"):
        ids = tuple(map(lambda instance: getattr(instance, id_field_name), instances))
        ids = tuple(filter(lambda x: x is not None, ids))
        distinct = set(ids)
        if len(ids) != len(distinct):
            errmsg = "Detect duplicate IDs from application caller"
            raise ValueError(errmsg)

    def save_with_rand_id(self, save_instance_fn, objs):
        self._assert_any_dup_id(objs)
        try:
            self._set_random_id(objs, self.MAX_GAP_VALUE)
            result = save_instance_fn()
        except self.expected_db_errors() as e:
            if self.is_db_err_recoverable(error=e):
                gap_ranges = self.get_gap_ranges(max_value=self.MAX_GAP_VALUE)
                assert any(gap_ranges), "no gap ranges found"
                error = e
                while (
                    True
                ):  # may try different ID number in case race condition happens
                    # find out the objects which have duplicate id, then give each of them distinct ID number
                    dup_id = self.extract_dup_id_from_error(error)
                    try:  # current id is duplicate, change to another one
                        self._rand_gap_id(objs, gap_ranges, dup_id=dup_id)
                        result = save_instance_fn()
                    except self.expected_db_errors() as e2:
                        # concurrent client requests happens to contend for the same ID number,
                        # however only one request succeed to gain the number as its new ID,
                        # and rest of the requests will have to try other different ID numbers
                        # in next iteration.
                        if self.is_db_err_recoverable(error=e2):
                            error = e2  # then try again
                        else:
                            raise
                    else:  # succeed to get the ID number
                        break
            else:
                raise
        return result

    def _set_random_id(self, instances, max_value, id_field_name="id"):
        objs_pk_null = filter(
            lambda instance: getattr(instance, id_field_name, None) is None, instances
        )
        for instance in objs_pk_null:
            rand_value = random.randrange(max_value)
            setattr(instance, id_field_name, rand_value)

    def get_gap_ranges(self, max_value):
        """
        return pairs of range value available for assigning numeric ID to new instance
        of ORM model class , each of which has the format (`lowerbound`, `upperbound`)
        """
        if not hasattr(self, "_gap_ranges"):
            model_cls = self.orm_model_class
            db_table = self.get_db_table_name(model_cls)
            # TODO, figure out how to support multi-column primary key
            pk_db_column = self.get_pk_db_column(model_cls)
            raw_sql_queries = get_sql_table_pk_gap_ranges(
                db_table=db_table, pk_db_column=pk_db_column, max_value=max_value
            )
            # execute 3 SELECT statements in one round trip to database server
            self._gap_ranges = self.low_lvl_get_gap_range(raw_sql_queries)
            # in case race condition happens to concurrent requests
            # asking for the same ID number
            self._recent_invalid_ids = []
        return self._gap_ranges

    def clean_gap_ranges(self, max_value):
        if hasattr(self, "_gap_ranges"):
            delattr(self, "_gap_ranges")
        if hasattr(self, "_recent_invalid_ids"):
            delattr(self, "_recent_invalid_ids")

    def _rand_gap_id(self, instances, gap_ranges, dup_id, id_field_name="id"):
        chosen_id = 0
        find_dup_obj = lambda obj: getattr(obj, id_field_name) == dup_id
        dup_instance = tuple(filter(find_dup_obj, instances))
        dup_instance = dup_instance[0]
        while chosen_id == 0:
            lower, upper = random.choice(gap_ranges)
            if lower == upper:
                chosen_id = lower
            else:
                chosen_id = random.randrange(start=lower, stop=upper + 1)
            if chosen_id in self._recent_invalid_ids:
                chosen_id = 0
        old_id = getattr(dup_instance, id_field_name)
        self._recent_invalid_ids.append(old_id)
        setattr(dup_instance, id_field_name, chosen_id)

    def expected_db_errors(self):
        raise NotImplementedError()

    def is_db_err_recoverable(self, error) -> bool:
        raise NotImplementedError()

    def low_lvl_get_gap_range(self, raw_sql_queries) -> list:
        raise NotImplementedError()

    def get_pk_db_column(self, model_cls) -> str:
        raise NotImplementedError()

    def get_db_table_name(self, model_cls) -> str:
        raise NotImplementedError()

    def extract_dup_id_from_error(self, error):
        raise NotImplementedError()


## end of class IdGapNumberFinderMixin
