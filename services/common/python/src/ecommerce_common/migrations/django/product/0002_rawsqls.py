# Generated by Django 3.1 on 2021-10-07 07:43
import json
from django.conf import settings as django_settings
from django.db import migrations, models

proj_path = django_settings.BASE_DIR
secrets_path = proj_path.joinpath("common/data/secrets.json")
secrets = None

with open(secrets_path, "r") as f:
    secrets = json.load(f)
    secrets = secrets["backend_apps"]["databases"]["default"]


def _mariadb_grant_priv(table_name):
    sql_pattern = "GRANT SELECT ON `%s`  TO '%s'@'%s'"
    reverse_sql_pattern = "REVOKE SELECT ON `%s`   FROM '%s'@'%s'"
    sql = sql_pattern % (table_name, secrets["USER"], secrets["HOST"])
    reverse_sql = reverse_sql_pattern % (table_name, secrets["USER"], secrets["HOST"])
    ops = migrations.RunSQL(sql=sql, reverse_sql=reverse_sql)
    return ops


table_names = (
    "django_migrations",
    "django_content_type",
)
rawsql_ops = map(_mariadb_grant_priv, table_names)


class Migration(migrations.Migration):
    dependencies = [
        ("product", "0001_initial"),
    ]
    operations = [op for op in rawsql_ops]
