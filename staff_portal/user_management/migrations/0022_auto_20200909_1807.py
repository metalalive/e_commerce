# Generated by Django 3.1 on 2020-09-09 10:07

from django.db import migrations, models


class Migration(migrations.Migration):

    dependencies = [
        ('user_management', '0021_auto_20200909_1754'),
    ]

    operations = [
        migrations.AddConstraint(
            model_name='userquotarelation',
            constraint=models.UniqueConstraint(fields=('user_id', 'user_type', 'usage_type'), name='unique_user_quota'),
        ),
    ]
