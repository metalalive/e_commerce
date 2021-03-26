from django.db.models.enums import IntegerChoices

class UnitOfMeasurement(IntegerChoices):
    # unit , countable object
    UNIT  = 0x0001
    DOZEN = 0x0002
    # working time
    DAY    = 0x0041
    HOUR   = 0x0042
    MINUTE = 0x0043
    SECOND = 0x0044
    WEEK   = 0x0045
    MONTH  = 0x0046
    YEAR   = 0x0047
    # weight
    GRAM      = 0x0081
    KILOGRAM  = 0x0082
    TONNE     = 0x0083
    MEGATONNE = 0x0084
    GIGATONNE = 0x0085
    MILLIGRAM = 0x0086
    MICROGRAM = 0x0087
    NANOGRAM  = 0x0088
    US_TON    = 0x0089
    UK_TON    = 0x008A
    LB_POUND  = 0x008B
    OZ_OUNCE  = 0x008C
    # length / distance
    CENTIMETER = 0x00C1
    METER      = 0x00C2
    KILOMETER  = 0x00C3
    MILLIMETER = 0x00C4
    NANOMETER  = 0x00C5
    INCH = 0x00C6
    FOOT = 0x00C7
    YARD = 0x00C8
    STATUTE_MILE  = 0x00C9
    NAUTICAL_MILE = 0x00CA
    # volume (liquid)
    LITRE       = 0x0101
    FLUID_OUNCE = 0x0102
    MILLILITER  = 0x0103
    BARREL      = 0x0104
    QUART_IMPERIAL = 0x0105
    QUART_US   = 0x0106
    QUART_UK   = 0x0107
    TEASPOON   = 0x0108
    TABLESPOON_US = 0x0109
    TABLESPOON_IMPERIAL = 0x010A
    GALLON_US     = 0x010B
    GALLON_IMPERIAL = 0x010C


