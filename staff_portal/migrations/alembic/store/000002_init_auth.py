"""init low-level permissions and quota materials

Revision ID: 000002
Revises: 
Create Date: 2021-11-10 23:39:21.898305

"""
from alembic import op
import sqlalchemy as sa
from sqlalchemy.sql import text as sa_text

from common.models.enums.base import AppCodeOptions
from store.models import StoreProfile, StoreStaff, StoreEmail, StorePhone, StoreProductAvailable

# revision identifiers, used by Alembic.
revision = '000002'
# In this project, multiple roots exist, this is new initial migration for different
# database, which does NOT depend on order app. Don't set '000001'
down_revision = None
branch_labels = None
depends_on = None

app_label = 'store'
app_code = getattr(AppCodeOptions, app_label).value[0]
actions = ('add', 'change', 'delete', 'view')

resource_classes = (StoreProfile, StoreProductAvailable)
auth_resource_labels = tuple(map(lambda cls:cls.__name__.lower(), resource_classes))

quota_mat_classes = (StoreProfile, StoreStaff, StoreEmail, StorePhone, StoreProductAvailable)
quota_mat_code = tuple(map(lambda cls: cls.quota_material.value, quota_mat_classes))


def upgrade():
    insert_contenttype_pattern = 'INSERT INTO `django_content_type` (`app_label`, `model`) VALUES (\'{app_label}\', \'%s\')'
    insert_contenttype_pattern = insert_contenttype_pattern.format(app_label=app_label)
    sqls = [insert_contenttype_pattern % label for label in auth_resource_labels]
    _run_sql(sqls)
    insert_perm_pattern = 'INSERT INTO `auth_permission` (`name`, `codename`, `content_type_id`) VALUES (\'%s\', \'%s\', (SELECT `id` FROM `django_content_type` WHERE `app_label` = \'{app_label}\' AND `model` = \'%s\'))'
    insert_perm_pattern = insert_perm_pattern.format(app_label=app_label)
    sqls = []
    for action in actions:
        for label in auth_resource_labels:
            codename = '%s_%s' % (action, label)
            name = 'Can %s %s' % (action, label)
            sql = insert_perm_pattern % (name, codename, label)
            sqls.append(sql)
    _run_sql(sqls)
    insert_quota_mat_pattern = 'INSERT INTO `quota_material` (`app_code`, `mat_code`) VALUES (%s, %s)'
    sqls = [insert_quota_mat_pattern % (app_code, mat_code) for mat_code in quota_mat_code]
    _run_sql(sqls)


def downgrade():
    delete_quota_mat_pattern = 'DELETE FROM `quota_material` WHERE `app_code` = %s AND `mat_code` = %s'
    sqls = [delete_quota_mat_pattern % (app_code, mat_code) for mat_code in quota_mat_code]
    _run_sql(sqls)
    delete_perm_pattern = 'DELETE FROM `auth_permission` WHERE `content_type_id` IN (SELECT `id` FROM `django_content_type` WHERE `app_label` = \'{app_label}\' AND `model` = \'%s\')'
    delete_perm_pattern = delete_perm_pattern.format(app_label=app_label)
    sqls = [delete_perm_pattern % label for label in auth_resource_labels]
    _run_sql(sqls)
    delete_contenttype_pattern = 'DELETE FROM `django_content_type` WHERE `app_label` = \'{app_label}\' AND `model` = \'%s\''
    delete_contenttype_pattern = delete_contenttype_pattern.format(app_label=app_label)
    sqls = [delete_contenttype_pattern % label for label in auth_resource_labels]
    _run_sql(sqls)


def _run_sql(sqls):
    conn = op.get_bind()
    for sql in sqls:
        conn.execute(sa_text(sql))


