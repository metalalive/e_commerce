"""create-all-tables

Revision ID: 000001
Revises: 
Create Date: 2024-05-05 02:48:02.526442

"""

from alembic import op
import sqlalchemy as sa
from sqlalchemy.dialects import mysql

# revision identifiers, used by Alembic.
revision = "000001"
down_revision = None
branch_labels = None
depends_on = None


def upgrade():
    # ### commands auto generated by Alembic - please adjust! ###
    op.create_table(
        "store_profile",
        sa.Column(
            "id", mysql.INTEGER(unsigned=True), autoincrement=False, nullable=False
        ),
        sa.Column("label", sa.String(length=32), nullable=False),
        sa.Column(
            "supervisor_id",
            mysql.INTEGER(unsigned=True),
            autoincrement=False,
            nullable=True,
        ),
        sa.Column("active", sa.Boolean(), nullable=True),
        sa.PrimaryKeyConstraint("id"),
    )
    op.create_table(
        "hour_of_operation",
        sa.Column("store_id", mysql.INTEGER(unsigned=True), nullable=False),
        # fmt: off
        sa.Column(
            "day",
            sa.Enum("SUNDAY", "MONDAY", "TUESDAY", "WEDNESDAY", "THURSDAY", "FRIDAY", "SATURDAY", name="enumweekday"),
            nullable=False,
        ),
        # fmt: on
        sa.Column("time_open", sa.Time(), nullable=False),
        sa.Column("time_close", sa.Time(), nullable=False),
        sa.ForeignKeyConstraint(["store_id"], ["store_profile.id"], ondelete="CASCADE"),
        sa.PrimaryKeyConstraint("store_id", "day"),
    )
    op.create_table(
        "outlet_location",
        sa.Column("store_id", mysql.INTEGER(unsigned=True), nullable=False),
        # fmt: off
        sa.Column(
            "country",
            sa.Enum(
                "AU", "AT", "CZ", "DE", "HK", "IN", "ID", "IL",
                "MY", "NZ", "PT", "SG", "TH", "TW", "US",
                name="countrycodeenum"),
            nullable=False,
        ),
        # fmt: on
        sa.Column("locality", sa.String(length=50), nullable=False),
        sa.Column("street", sa.String(length=50), nullable=False),
        sa.Column("detail", sa.Text(), nullable=True),
        sa.Column("floor", sa.SmallInteger(), nullable=False),
        sa.ForeignKeyConstraint(["store_id"], ["store_profile.id"], ondelete="CASCADE"),
        sa.PrimaryKeyConstraint("store_id"),
    )
    op.create_table(
        "store_email",
        sa.Column("store_id", mysql.INTEGER(unsigned=True), nullable=False),
        sa.Column("seq", sa.SmallInteger(), autoincrement=False, nullable=False),
        sa.Column("addr", sa.String(length=160), nullable=False),
        sa.ForeignKeyConstraint(["store_id"], ["store_profile.id"], ondelete="CASCADE"),
        sa.PrimaryKeyConstraint("store_id", "seq"),
    )
    op.create_table(
        "store_phone",
        sa.Column("store_id", mysql.INTEGER(unsigned=True), nullable=False),
        sa.Column("seq", sa.SmallInteger(), autoincrement=False, nullable=False),
        sa.Column("country_code", sa.String(length=3), nullable=False),
        sa.Column("line_number", sa.String(length=15), nullable=False),
        sa.ForeignKeyConstraint(["store_id"], ["store_profile.id"], ondelete="CASCADE"),
        sa.PrimaryKeyConstraint("store_id", "seq"),
    )
    op.create_table(
        "store_product_available",
        sa.Column("store_id", mysql.INTEGER(unsigned=True), nullable=False),
        sa.Column(
            "product_type",
            sa.Enum("ITEM", "PACKAGE", name="saleabletypeenum"),
            nullable=False,
        ),
        sa.Column("product_id", mysql.INTEGER(unsigned=True), nullable=False),
        sa.Column("price", mysql.INTEGER(unsigned=True), nullable=False),
        sa.Column("start_after", sa.DateTime(), nullable=True),
        sa.Column("end_before", sa.DateTime(), nullable=True),
        sa.ForeignKeyConstraint(["store_id"], ["store_profile.id"], ondelete="CASCADE"),
        sa.PrimaryKeyConstraint("store_id", "product_type", "product_id"),
    )
    op.create_table(
        "store_staff",
        sa.Column("store_id", mysql.INTEGER(unsigned=True), nullable=False),
        sa.Column(
            "staff_id",
            mysql.INTEGER(unsigned=True),
            autoincrement=False,
            nullable=False,
        ),
        sa.Column("start_after", sa.DateTime(), nullable=True),
        sa.Column("end_before", sa.DateTime(), nullable=True),
        sa.ForeignKeyConstraint(["store_id"], ["store_profile.id"], ondelete="CASCADE"),
        sa.PrimaryKeyConstraint("store_id", "staff_id"),
    )
    # ### end Alembic commands ###


def downgrade():
    # ### commands auto generated by Alembic - please adjust! ###
    op.drop_table("store_staff")
    op.drop_table("store_product_available")
    op.drop_table("store_phone")
    op.drop_table("store_email")
    op.drop_table("outlet_location")
    op.drop_table("hour_of_operation")
    op.drop_table("store_profile")
    # ### end Alembic commands ###
