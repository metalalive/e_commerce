"""remove_product_type

Revision ID: 000003
Revises: 000002
Create Date: 2025-01-25 14:46:59.897373

"""

from alembic import op
import sqlalchemy as sa
from sqlalchemy.dialects import mysql

# revision identifiers, used by Alembic.
revision = "000003"
down_revision = "000002"
branch_labels = None
depends_on = "000002"

CSTR_NAME = "store_product_available_ibfk_1"
table_name = "store_product_available"


def upgrade():
    op.drop_constraint(CSTR_NAME, table_name, type_="foreignkey")
    op.drop_constraint(None, table_name, type_="primary")
    op.drop_column(table_name, "product_type")

    op.create_primary_key(None, table_name, columns=["store_id", "product_id"])
    op.create_foreign_key(
        constraint_name=CSTR_NAME,
        source_table=table_name,
        referent_table="store_profile",
        local_cols=["store_id"],
        remote_cols=["id"],
        ondelete="CASCADE",
    )


def downgrade():
    op.drop_constraint(CSTR_NAME, table_name, type_="foreignkey")
    op.drop_constraint(None, table_name, type_="primary")
    op.add_column(
        table_name,
        sa.Column("product_type", mysql.ENUM("ITEM", "PACKAGE"), nullable=False),
    )
    op.create_primary_key(
        None, table_name, columns=["store_id", "product_type", "product_id"]
    )
    op.create_foreign_key(
        constraint_name=CSTR_NAME,
        source_table=table_name,
        referent_table="store_profile",
        local_cols=["store_id"],
        remote_cols=["id"],
        ondelete="CASCADE",
    )
