"""add_attris_charge

Revision ID: 000004
Revises: 000003
Create Date: 2025-01-26 00:11:12.225822

"""

from alembic import op
import sqlalchemy as sa
from sqlalchemy.dialects import mysql

# revision identifiers, used by Alembic.
revision = "000004"
down_revision = "000003"
branch_labels = None
depends_on = "000003"


def upgrade():
    conn = op.get_bind()
    conn.execute(
        sa.sql.text(
            "ALTER TABLE `store_product_available` RENAME COLUMN `price` TO `base_price`"
        )
    )
    op.add_column(
        "store_product_available",
        sa.Column("attrs_charge", mysql.JSON(), nullable=False),
    )
    op.add_column(
        "store_product_available",
        sa.Column("attrs_last_update", sa.DateTime(), nullable=False),
    )


def downgrade():
    op.drop_column("store_product_available", "attrs_last_update")
    op.drop_column("store_product_available", "attrs_charge")
    conn = op.get_bind()
    conn.execute(
        sa.sql.text(
            "ALTER TABLE `store_product_available` RENAME COLUMN `base_price` TO `price`"
        )
    )
