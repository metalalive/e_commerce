import django.contrib.contenttypes.models
from django.db import migrations, models
from common.models.migrations import AlterTablePrivilege


class Migration(migrations.Migration):

    dependencies = [
    ]

    operations = [
        migrations.CreateModel(
            name='ContentType',
            fields=[
                ('id', models.AutoField(verbose_name='ID', serialize=False, auto_created=True, primary_key=True)),
                ('name', models.CharField(max_length=100)),
                ('app_label', models.CharField(max_length=100)),
                ('model', models.CharField(max_length=100, verbose_name='python model class name')),
            ],
            options={
                'ordering': ('name',),
                'db_table': 'django_content_type',
                'verbose_name': 'content type',
                'verbose_name_plural': 'content types',
            },
            bases=(models.Model,),
            managers=[
                ('objects', django.contrib.contenttypes.models.ContentTypeManager()),
            ],
        ),
        migrations.AlterUniqueTogether(
            name='contenttype',
            unique_together={('app_label', 'model')},
        ),
    ]

    def __new__(cls, *args, **kwargs):
        if not hasattr(cls, '_privilege_update_init'):
            cls.operations[0]._priv_lvl = AlterTablePrivilege.PRIVILEGE_MAP['READ_ONLY']
            #cls.operations[0]._priv_lvl = AlterTablePrivilege.PRIVILEGE_MAP['READ_WRITE']
            privilege_update_obj = AlterTablePrivilege( autogen_ops=cls.operations,  db_setup_tag='default')
            privilege_update_obj = AlterTablePrivilege( autogen_ops=cls.operations,  db_setup_tag='product_dev_service')
            privilege_update_obj = AlterTablePrivilege( autogen_ops=cls.operations,  db_setup_tag='usermgt_service')
            cls._privilege_update_init = True
        return super().__new__(cls)

