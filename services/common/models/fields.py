from functools import partial
# TODO: rename this module to django.fields
from django.db import models
from django.utils.translation import gettext_lazy as _
from django.utils.functional import  cached_property

from common.util.python import flatten_nested_iterable


def monkeypatch_sqlcompiler():
    from django.db.models.sql.compiler import SQLCompiler
    old_get_default_columns = SQLCompiler.get_default_columns

    if hasattr(old_get_default_columns, '_patched'):
        return # skip, already monkey-patched

    from django.db.models.constants import LOOKUP_SEP
    from django.db.models.sql.query import get_order_dir
    from django.core.exceptions import FieldError
    from django.db.models.expressions import OrderBy
    def patched_get_default_columns(self, start_alias=None, opts=None, from_parent=None):
        result = old_get_default_columns(self, start_alias=start_alias,
                opts=opts, from_parent=from_parent)
        # there might be nested iterable item in the result list, which indicates that it is
        # composite key, so it is necessary to flatten the result list again.
        flatten = tuple(flatten_nested_iterable(list_=result))
        return flatten

    def patched_find_ordering_name(self, name, opts, alias=None, default_order='ASC',
                           already_seen=None):
        name, order = get_order_dir(name, default_order)
        descending = order == 'DESC'
        pieces = name.split(LOOKUP_SEP)
        field, targets, alias, joins, path, opts, transform_function = self._setup_joins(pieces, opts, alias)
        # If we get to this point and the field is a relation to another model,
        # append the default ordering for that model unless it is the pk
        # shortcut or the attribute name of the field that is specified.
        if field.is_relation and opts.ordering and getattr(field, 'attname', None) != name and name != 'pk':
            # Firstly, avoid infinite loops.
            already_seen = already_seen or set()
            join_tuple = tuple(getattr(self.query.alias_map[j], 'join_cols', None) for j in joins)
            if join_tuple in already_seen:
                raise FieldError('Infinite loop caused by ordering.')
            already_seen.add(join_tuple)

            results = []
            for item in opts.ordering:
                if hasattr(item, 'resolve_expression') and not isinstance(item, OrderBy):
                    item = item.desc() if descending else item.asc()
                if isinstance(item, OrderBy):
                    results.append((item, False))
                    continue
                results.extend(self.find_ordering_name(item, opts, alias,
                                                       order, already_seen))
            return results
        targets, alias, _ = self.query.trim_joins(targets, joins, path)
        # `targets` store list of items which represent column of database table, for
        # composite key, a target item could also be nested list of item, this monkey-patch
        # function is aimed at flattening any nested item in the `targets` list.
        columns = [transform_function(t, alias) for t in targets]
        columns = tuple(flatten_nested_iterable(list_=columns))
        return [(OrderBy(col, descending=descending), False) for col in columns]

    patched_get_default_columns._patched = True
    SQLCompiler.get_default_columns = patched_get_default_columns
    patched_find_ordering_name._patched = True
    SQLCompiler.find_ordering_name = patched_find_ordering_name
## end of monkeypatch_sqlcompiler()


def monkeypatch_sqlquery_build_filter():
    from django.db.models.sql.query import Query
    old_build_filter = Query.build_filter

    if hasattr(old_build_filter, '_patched'):
        return # skip, already monkey-patched

    from collections.abc import Iterator, Iterable
    from django.core.exceptions import FieldError
    from django.db.models import Q
    from django.db.models.constants import LOOKUP_SEP
    from django.db.models.fields.related_lookups import MultiColSource
    from django.db.models.sql.where import (AND, OR, ExtraWhere, NothingNode, WhereNode,)
    from django.db.models.sql.datastructures import (BaseTable, Empty, Join, MultiJoin,)
    from django.db.models.sql.constants import INNER, LOUTER, ORDER_DIR, SINGLE

    def _build_filter_by_expression(self, filter_expr, allow_joins):
        if not getattr(filter_expr, 'conditional', False):
            raise TypeError('Cannot filter against a non-conditional expression.')
        condition_ = self.build_lookup(
            ['exact'], filter_expr.resolve_expression(self, allow_joins=allow_joins), True
        )
        clause = self.where_class()
        clause.add(condition_, AND)
        return clause, []

    def patched_build_filter(self, filter_expr, branch_negated=False, current_negated=False,
                     can_reuse=None, allow_joins=True, split_subq=True,
                     reuse_with_filtered_relation=False, check_filterable=True):
        if isinstance(filter_expr, dict):
            raise FieldError("Cannot parse keyword query as dict")
        if isinstance(filter_expr, Q):
            return self._add_q(
                filter_expr,  branch_negated=branch_negated, current_negated=current_negated,
                used_aliases=can_reuse, allow_joins=allow_joins, split_subq=split_subq,
                check_filterable=check_filterable,  )
        if hasattr(filter_expr, 'resolve_expression'):
            return self._build_filter_by_expression(filter_expr, allow_joins)
        arg, value = filter_expr
        if not arg:
            raise FieldError("Cannot parse keyword query %r" % arg)
        lookups, parts, reffed_expression = self.solve_lookup_type(arg)

        if check_filterable:
            self.check_filterable(reffed_expression)

        if not allow_joins and len(parts) > 1:
            raise FieldError("Joined field references are not permitted in this query")

        pre_joins = self.alias_refcount.copy()
        value = self.resolve_lookup_value(value, can_reuse, allow_joins)
        used_joins = {k for k, v in self.alias_refcount.items() if v > pre_joins.get(k, 0)}

        if check_filterable:
            self.check_filterable(value)

        clause = self.where_class()
        if reffed_expression:
            condition_ = self.build_lookup(lookups, reffed_expression, value)
            clause.add(condition_, AND)
            return clause, []

        opts = self.get_meta()
        alias = self.get_initial_alias()
        allow_many = not branch_negated or not split_subq
        try:
            join_info = self.setup_joins(
                parts, opts, alias, can_reuse=can_reuse, allow_many=allow_many,
                reuse_with_filtered_relation=reuse_with_filtered_relation,
            )

            # Prevent iterator from being consumed by check_related_objects()
            if isinstance(value, Iterator):
                value = list(value)
            self.check_related_objects(join_info.final_field, value, join_info.opts)

            # split_exclude() needs to know which joins were generated for the
            # lookup parts
            self._lookup_joins = join_info.joins
        except MultiJoin as e:
            return self.split_exclude(filter_expr, can_reuse, e.names_with_path)

        # Update used_joins before trimming since they are reused to determine
        # which joins could be later promoted to INNER.
        used_joins.update(join_info.joins)
        targets, alias, join_list = self.trim_joins(join_info.targets, join_info.joins, join_info.path)
        if can_reuse is not None:
            can_reuse.update(join_list)

        if join_info.final_field.is_relation:
            # No support for transforms for relational fields
            num_lookups = len(lookups)
            if num_lookups > 1:
                raise FieldError('Related Field got invalid lookup: {}'.format(lookups[0]))
            if len(targets) == 1:
                col = self._get_col(targets[0], join_info.final_field, alias)
            else:
                col = MultiColSource(alias, targets, join_info.targets, join_info.final_field)
        else:
            col = self._get_col(targets[0], join_info.final_field, alias)

        # implicitly means it is multi-column key
        if isinstance(col, Iterable) and not isinstance(col, str):
            valid_fd_names = map(lambda c: (c.target.name, c.target.attname), col)
            valid_fd_names = tuple(flatten_nested_iterable(list_=valid_fd_names))
            parent_related_fields = parts.copy()
            pk_field = self.model._meta.pk
            possible_pk_names = ('pk', pk_field.name, )
            if parent_related_fields[-1] in possible_pk_names:
                parent_related_fields.pop()
            # the value could also be list of dicts, queryset.bulk_update will also invoke
            # this function with pk__in condition, the monkeypatch code also needs to handle
            # such situation.
            if isinstance(value, (list,tuple)) and 'in' in lookups:
                # CAUTION: you must set connector (in where node) to OR , otherwise the
                # children list will implicitly add one extra empty node (AND clause),
                # then SQL compiler treats this empty node as FALSE in boolean value the
                # clause statement will be translated as :
                #      (EMPTY_NODE AND SUB_CLAUSE_1 AND SUB_CLAUSE_2 AND ...)
                # ==>  (FALSE AND SUB_CLAUSE_1 AND SUB_CLAUSE_2 AND ...)
                # ==>  << disappeared because it is optimized >>
                clause.connector = OR
                for val_item in value:
                    nested_where_clause = self.where_class(connector=OR, negated=current_negated )
                    self._build_filter_single_composite_pkey(value=val_item, col=col, parent_related_fields=parent_related_fields,
                            clause_out=nested_where_clause, valid_fd_names=valid_fd_names)
                    clause.add(nested_where_clause, OR)
            else: # must be dictionary
                self._build_filter_single_composite_pkey(value=value, col=col, parent_related_fields=parent_related_fields,
                        clause_out=clause, valid_fd_names=valid_fd_names)
            require_outer = False # TODO: currently not support isnull lookup
        else: # otherwise it is single-column key as usual
            condition_ = self.build_lookup(lookups, col, value)
            clause.add(condition_, AND)
            require_outer = self._build_filter_isnull_outer(condition_=condition_,
                    current_negated=current_negated, clause=clause, targets=targets,
                    join_list=join_list, join_info=join_info, alias=alias)

        return clause, used_joins if not require_outer else ()
    ## end of patched_build_filter()

    def _build_filter_single_composite_pkey(self, value:dict, col, parent_related_fields, clause_out, valid_fd_names):
        full_path_field = parent_related_fields
        for nested_arg, nested_value in value.items():
            full_path_field.append(nested_arg)
            nested_arg = LOOKUP_SEP.join(full_path_field)
            full_path_field.pop()
            lookup, fd_name, is_valid_name = self._build_filter_validate_fd_name(nested_arg, valid_fd_names)
            if not is_valid_name:
                raise FieldError('Multi-column Key Field got invalid column names : {}'.format(nested_arg))
            _fn_cond = lambda c: fd_name in [c.target.name, c.target.attname]
            chosen_col = tuple(filter(_fn_cond, col))
            condition_ = self.build_lookup(lookup, chosen_col[0], nested_value)
            clause_out.add(condition_, AND)

    def _build_filter_isnull_outer(self, condition_, current_negated, clause,
                targets,  join_list, join_info, alias):
        lookup_type = condition_.lookup_name
        require_outer = lookup_type == 'isnull' and condition_.rhs is True and not current_negated
        if current_negated and (lookup_type != 'isnull' or condition_.rhs is False) and condition_.rhs is not None:
            require_outer = True
            if (lookup_type != 'isnull' and (
                    self.is_nullable(targets[0]) or
                    self.alias_map[join_list[-1]].join_type == LOUTER)):
                # The condition_ added here will be SQL like this:
                # NOT (col IS NOT NULL), where the first NOT is added in
                # upper layers of code. The reason for addition is that if col
                # is null, then col != someval will result in SQL "unknown"
                # which isn't the same as in Python. The Python None handling
                # is wanted, and it can be gotten by
                # (col IS NULL OR col != someval)
                #   <=>
                # NOT (col IS NOT NULL AND col = someval).
                lookup_class = targets[0].get_lookup('isnull')
                col = self._get_col(targets[0], join_info.targets[0], alias)
                clause.add(lookup_class(col, False), AND)
        return require_outer

    def _build_filter_validate_fd_name(self, nested_arg, valid_field_names):
        # currently multi-column key ignores previously generated lookups,
        # and don't support reffed_expression
        lookup, parts, _ = self.solve_lookup_type(nested_arg)
        if parts: # `parts` includes optional list of related fields to final non-relation field
            is_valid = parts[-1] in valid_field_names
            part = parts[-1]
        else:
            is_valid = False
        return lookup, part, is_valid

    patched_build_filter._patched = True
    Query.build_filter = patched_build_filter
    Query._build_filter_by_expression = _build_filter_by_expression
    Query._build_filter_isnull_outer  = _build_filter_isnull_outer
    Query._build_filter_validate_fd_name = _build_filter_validate_fd_name
    Query._build_filter_single_composite_pkey = _build_filter_single_composite_pkey
## end of monkeypatch_sqlquery_build_filter()


def monkeypatch_sqlquery_add_fields():
    from django.db.models.sql.query import Query, get_field_names_from_opts
    old_fn = Query.add_fields
    if hasattr(old_fn, '_patched'):
        return # skip, already monkey-patched

    from django.db.models.constants import LOOKUP_SEP
    from django.db.models.sql.datastructures import MultiJoin
    from django.core.exceptions import FieldError
    def patched_add_fields(self, field_names, allow_m2m=True):
        alias = self.get_initial_alias()
        opts = self.get_meta()

        try:
            cols = []
            for name in field_names:
                # Join promotion note - we must not remove any rows here, so
                # if there is no existing joins, use outer join.
                join_info = self.setup_joins(name.split(LOOKUP_SEP), opts, alias, allow_many=allow_m2m)
                targets, final_alias, joins = self.trim_joins(
                    join_info.targets,
                    join_info.joins,
                    join_info.path,
                )
                for target in targets:
                    col = join_info.transform_function(target, final_alias)
                    if isinstance(col, (list,tuple)): # implicitly means a multi-column key
                        cols.extend(col)
                    else:
                        cols.append(col)
            if cols:
                self.set_select(cols)
        except MultiJoin:
            raise FieldError("Invalid field name: '%s'" % name)
        except FieldError:
            if LOOKUP_SEP in name:
                # For lookups spanning over relationships, show the error
                # from the model on which the lookup failed.
                raise
            else:
                names = sorted([
                    *get_field_names_from_opts(opts), *self.extra,
                    *self.annotation_select, *self._filtered_relations
                ])
                raise FieldError("Cannot resolve keyword %r into field. "
                                 "Choices are: %s" % (name, ", ".join(names)))
    # end of patched_add_fields()
    patched_add_fields._patched = True
    Query.add_fields = patched_add_fields
## end of monkeypatch_sqlquery_add_fields()


def monkeypatch_sql_insert_compiler():
    from django.db.models.sql.compiler import SQLInsertCompiler
    old_as_sql = SQLInsertCompiler.as_sql
    if hasattr(old_as_sql, '_patched'):
        return # skip, already monkey-patched

    def patched_as_sql(self):
        # We don't need quote_name_unless_alias() here, since these are all
        # going to be column names (so we can avoid the extra overhead).
        qn = self.connection.ops.quote_name
        opts = self.query.get_meta()
        insert_statement = self.connection.ops.insert_statement(ignore_conflicts=self.query.ignore_conflicts)
        result = ['%s %s' % (insert_statement, qn(opts.db_table))]
        fields = self.query.fields or [opts.pk]
        column_names  = list(flatten_nested_iterable(list_=[f.column for f in fields]))
        # remove duplicates (which comes from composite key) whilst preserving original order
        column_names  = list(dict.fromkeys(column_names))
        fields = tuple(filter(lambda f: f.column in column_names, fields))
        result.append('(%s)' % ', '.join(qn(col) for col in column_names))

        if self.query.fields:
            value_rows = [
                [self.prepare_value(field, self.pre_save_val(field, obj)) for field in fields]
                for obj in self.query.objs
            ]
        else:
            # An empty object.
            value_rows = [[self.connection.ops.pk_default_value()] for _ in self.query.objs]
            fields = [None]

        # Currently the backends just accept values when generating bulk
        # queries and generate their own placeholders. Doing that isn't
        # necessary and it should be possible to use placeholders and
        # expressions in bulk inserts too.
        can_bulk = (not self.returning_fields and self.connection.features.has_bulk_insert)

        placeholder_rows, param_rows = self.assemble_as_sql(fields, value_rows)

        ignore_conflicts_suffix_sql = self.connection.ops.ignore_conflicts_suffix_sql(
            ignore_conflicts=self.query.ignore_conflicts
        )
        if self.returning_fields and self.connection.features.can_return_columns_from_insert:
            if self.connection.features.can_return_rows_from_bulk_insert:
                result.append(self.connection.ops.bulk_insert_sql(fields, placeholder_rows))
                params = param_rows
            else:
                result.append("VALUES (%s)" % ", ".join(placeholder_rows[0]))
                params = [param_rows[0]]
            if ignore_conflicts_suffix_sql:
                result.append(ignore_conflicts_suffix_sql)
            # Skip empty r_sql to allow subclasses to customize behavior for
            # 3rd party backends. Refs #19096.
            r_sql, self.returning_params = self.connection.ops.return_insert_columns(self.returning_fields)
            if r_sql:
                result.append(r_sql)
                params += [self.returning_params]
            return [(" ".join(result), tuple(chain.from_iterable(params)))]

        if can_bulk:
            result.append(self.connection.ops.bulk_insert_sql(fields, placeholder_rows))
            if ignore_conflicts_suffix_sql:
                result.append(ignore_conflicts_suffix_sql)
            return [(" ".join(result), tuple(p for ps in param_rows for p in ps))]
        else:
            if ignore_conflicts_suffix_sql:
                result.append(ignore_conflicts_suffix_sql)
            return [
                (" ".join(result + ["VALUES (%s)" % ", ".join(p)]), vals)
                for p, vals in zip(placeholder_rows, param_rows)
            ]

    patched_as_sql._patched = True
    SQLInsertCompiler.as_sql = patched_as_sql
## end of monkeypatch_sql_insert_compiler


def monkeypatch_deferred_attr():
    from django.db.models.query_utils import DeferredAttribute
    old_get_attr = DeferredAttribute.__get__
    if hasattr(old_get_attr, '_patched'):
        return # skip, already monkey-patched

    def patched_get_attr(self, instance, cls=None):
        """
        Retrieve and caches the value from the datastore on the first lookup.
        Return the cached value.
        """
        if instance is None:
            return self
        data = instance.__dict__
        field_name = self.field.attname
        if self.field.get_internal_type() == 'CompoundPrimaryKeyField':
            return _get_composite_pk_val(model_instance=instance , field_instance=self.field)
        else: # original implementation code in Django
            if field_name not in data:
                # Let's see if the field is part of the parent chain. If so we
                # might be able to reuse the already loaded value. Refs #18343.
                val = self._check_parent_chain(instance)
                if val is None:
                    instance.refresh_from_db(fields=[field_name])
                    val = getattr(instance, field_name)
                data[field_name] = val
            return data[field_name]
    patched_get_attr._patched = True
    DeferredAttribute.__get__ = patched_get_attr
## end of monkeypatch_deferred_attr()


def monkeypatch_django_model_ops():
    from django.db.models import Model as DjangoModel
    old_save_base = DjangoModel.save_base
    old_hash_fn = DjangoModel.__hash__
    if hasattr(old_save_base, '_patched'):
        return # skip, already monkey-patched

    import json
    def patched_save_base(self, *args, **kwargs):
        result = old_save_base(self, *args, **kwargs)
        # When application caller invokes save() on a model instance, Django defaults to
        # check pk property in order to determine weather to perform insertion or update
        # for current model instance. However, a model instance with composite pk is NOT
        # allowed applications to edit its pk property ...(TODO)
        pk_field = self._meta.pk
        if pk_field.get_internal_type() == 'CompoundPrimaryKeyField':
            self.pk = _get_composite_pk_val(model_instance=self , field_instance=pk_field)
        return result

    def patched_hash_fn(self):
        if self.pk is None:
            raise TypeError("Model instances without primary key value are unhashable")
        if isinstance(self.pk, dict): # implicitly means it is composite pk
            serialized = json.dumps(self.pk, sort_keys=True)
            hashed = hash(serialized)
        else:
            hashed =  hash(self.pk)
        return hashed

    patched_save_base._patched = True
    patched_hash_fn._patched = True
    DjangoModel.save_base = patched_save_base
    DjangoModel.__hash__ = patched_hash_fn


def monkeypatch_queryset_bulk_ops():
    from django.db.models.query import QuerySet as DjangoQuerySet
    old_bulk_create = DjangoQuerySet.bulk_create
    old_bulk_update = DjangoQuerySet.bulk_update
    if hasattr(old_bulk_create, '_patched'):
        return # skip, already monkey-patched

    from django.utils.functional import partition
    def _update_composite_pk_after_bulk_ops(objs, only_for_nonpk):
        if not any(objs):
            return
        pk_field = objs[0]._meta.pk # there must be at least one instance
        if pk_field.get_internal_type() == 'CompoundPrimaryKeyField':
            if only_for_nonpk:
                _, objs_without_pk = partition(lambda o: o.pk is None, objs)
                filtered_objs = objs_without_pk
            else:
                filtered_objs = objs
                pass
            for model_obj in filtered_objs:
                field_obj = model_obj._meta.pk
                model_obj.pk = _get_composite_pk_val(model_instance=model_obj, field_instance=field_obj)

    def patched_bulk_update(self, objs, *args, **kwargs):
        result = old_bulk_update(self, objs, *args, **kwargs)
        _update_composite_pk_after_bulk_ops(objs, only_for_nonpk=False)
        return result

    def patched_bulk_create(self, objs, *args, **kwargs):
        result = old_bulk_create(self, objs, *args, **kwargs)
        _update_composite_pk_after_bulk_ops(objs, only_for_nonpk=True)
        return result
    patched_bulk_create._patched = True
    patched_bulk_update._patched = True
    DjangoQuerySet.bulk_create = patched_bulk_create
    DjangoQuerySet.bulk_update = patched_bulk_update
## end of monkeypatch_queryset_bulk_ops()


def _get_composite_pk_val(model_instance, field_instance):
    composite_field_names = map(lambda f: f.name , field_instance._composite_fields)
    def _composite_fields_to_pk_val(fname):
        value = getattr(model_instance, fname)
        if isinstance(value, models.Model):
            value = value.pk
        return (fname, value)
    pk_values = map(_composite_fields_to_pk_val, composite_field_names)
    return dict(pk_values)


def monkeypatch_queryset_lookup_values():
    from django.db.models.query import QuerySet as DjangoQuerySet
    old__values_fn = DjangoQuerySet._values
    if hasattr(old__values_fn, '_patched'):
        return

    from django.db.models.constants import LOOKUP_SEP
    from django.db.models.fields.related_descriptors import ReverseManyToOneDescriptor
    def _parse_compo_pk_related_fields(self, fields_name):
        # note this function does not handle composite pk field ended with `id` and `pk`
        parsed_fields = []
        for field_name in fields_name:
            curr_traced_model = self.model
            related_field_chain = field_name.split(LOOKUP_SEP)
            def _extract_compo_pk_cols_fn(last_field):
                chain_cp = related_field_chain.copy()
                chain_cp.append(last_field)
                return LOOKUP_SEP.join(chain_cp)
            for rel_fd in related_field_chain:
                fd_descriptor = getattr(curr_traced_model, rel_fd)
                if isinstance(fd_descriptor, (ReverseManyToOneDescriptor,)):
                    # each related field comes from either current model or related model,
                    # which depends on how Django models and their relations are declared
                    # in applications, so I simply check whether `related_model` or `model`
                    # field is the same as currently found model.
                    if curr_traced_model != fd_descriptor.field.related_model:
                        curr_traced_model = fd_descriptor.field.related_model
                    else:
                        curr_traced_model = fd_descriptor.field.model
                    if rel_fd == related_field_chain[-1]:
                        pk_field = curr_traced_model._meta.pk
                        is_compo_pk = pk_field.get_internal_type() == 'CompoundPrimaryKeyField'
                        if is_compo_pk:
                            compo_fnames = [f.attname for f in pk_field._composite_fields]
                            compo_fnames = list(map(_extract_compo_pk_cols_fn, compo_fnames))
                            parsed_fields.extend(compo_fnames)
                        else:
                            parsed_fields.append(field_name)
                else:
                    parsed_fields.append(field_name)
                    break
        return parsed_fields

    def patched__values_fn(self, *fields, **expressions):
        pk_field = self.model._meta.pk
        is_compo_pk = pk_field.get_internal_type() == 'CompoundPrimaryKeyField'
        if is_compo_pk:
            if not fields: # note : don't use set, use tuple to remain the order of fields
                # TODO, figure out why Django uses f.attname instead of f.name
                fields = tuple([f.attname for f in self.model._meta.concrete_fields])
            compo_fnames = [f.attname for f in pk_field._composite_fields]
            origin_fields = fields
            replaced = [compo_fnames  if fname in (pk_field.attname, 'pk') \
                    else fname for fname in origin_fields]
            flattened = flatten_nested_iterable(list_=replaced)
            fields = tuple(dict.fromkeys(flattened))
        else:
            fields = self._parse_compo_pk_related_fields(fields)
            #pass
        return old__values_fn(self, *fields, **expressions)

    patched__values_fn._patched = True
    DjangoQuerySet._values = patched__values_fn
    DjangoQuerySet._parse_compo_pk_related_fields = _parse_compo_pk_related_fields
## end of monkeypatch_queryset_lookup_values()


def monkeypatch_aggregate_query():
    from django.db.models.sql.subqueries import AggregateQuery
    old_add_subquery = AggregateQuery.add_subquery
    if hasattr(old_add_subquery, '_patched'):
        return
    def patched_add_subquery(self, query, using):
        pk_field = query.model._meta.pk
        if pk_field.get_internal_type() == 'CompoundPrimaryKeyField':
            # flatten nested item in select list if found
            flattened = flatten_nested_iterable(list_=query.select)
            query.select = tuple(flattened)
        return old_add_subquery(self, query, using)

    patched_add_subquery._patched = True
    AggregateQuery.add_subquery = patched_add_subquery
## end of monkeypatch_aggregate_query()


monkeypatch_sqlcompiler()
monkeypatch_sqlquery_build_filter()
monkeypatch_sqlquery_add_fields()
monkeypatch_sql_insert_compiler()
monkeypatch_deferred_attr()
monkeypatch_django_model_ops()
monkeypatch_queryset_bulk_ops()
monkeypatch_queryset_lookup_values()
monkeypatch_aggregate_query()


class CompoundPrimaryKeyField(models.Field):
    empty_strings_allowed = False
    default_error_messages = {
        'invalid': _('“%(value)s” value must be a set of existing fields.'),
    }
    description = _("CompoundPrimaryKey")

    def __init__(self, inc_fields, **kwargs):
        assert any(inc_fields), '`fields` argument has to be non-empty list'
        # limitation: the fields name in composite pk have to be the same as the
        # columns name in database table, because Field.get_attname_column() will
        # be frequently invoked before django loads all necessary model classes.
        self._inc_fields_name = inc_fields
        kwargs['primary_key'] = True
        kwargs['auto_created'] = False
        kwargs['editable'] = False
        kwargs.pop('db_column', None)
        super().__init__(**kwargs)
        self.db_column = self._composite_columns() # TODO, seperate db columns from field name

    def get_internal_type(self):
        # fixed class name for all subclasses
        return "CompoundPrimaryKeyField"

    def deconstruct(self):
        name, path, args, kwargs = super().deconstruct()
        kwargs['inc_fields'] = self._inc_fields_name
        return name, path, args, kwargs

    def db_type(self, connection):
        """
        return data type of column in database table
        """
        # this field returns any value other than `None` to skip check
        # when creating database table
        return ''

    def db_check(self, connection):
        # return extra check of SQL clause on this field
        pass

    @property
    def db_columns(self):
        return self.db_column

    def _composite_columns(self):
        return self._inc_fields_name

    @property
    def _composite_fields(self):
        out = []
        for fd in self.model._meta.local_fields:
            if type(self).__name__ == fd.get_internal_type():
                continue
            if fd.name in self._composite_columns():
                out.append(fd)
        return out

    def get_col(self, alias, output_field=None):
        #if output_field is not None:
        #    import pdb
        #    pdb.set_trace()
        if output_field is None:
            output_field = self
        if alias != self.model._meta.db_table or output_field != self:
            gen_col_helper = partial(_create_col, alias=alias, output_field=output_field)
            return list(map(gen_col_helper, self._composite_fields))
        else:
            return self.cached_col

    @cached_property
    def cached_col(self):
        gen_col_helper = partial(_create_col, alias=self.model._meta.db_table,
                output_field=None)
        return list(map(gen_col_helper, self._composite_fields))
## end of class CompoundPrimaryKeyField


def _create_col(target, alias, output_field):
    # `target` has to be at first position  for map() function
    try:
        Col
    except NameError as e:
        from django.db.models.expressions import Col
    return  Col(alias, target, output_field)


