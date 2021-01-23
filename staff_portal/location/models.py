from django.db import models

# record locations for generic business operations,
# e.g.
# your businuss may need to record :
#  * one or more outlets, for selling goods to end customers
#  * warehouse, you either rent a space from others, or build your own
#  * factory, if your finished goods is manufactured by your own company
#  * farm, in case your company contracts farmers who grow produce (raw
#    materials) for product manufacture.
#  * shipping addresses of customers and suppliers


class Location(models.Model):
    class Meta:
        db_table = 'location'

    class CountryCode(models.TextChoices):
        AU = 'AU',
        AT = 'AT',
        CZ = 'CZ',
        DE = 'DE',
        HK = 'HK',
        IN = 'IN',
        ID = 'ID',
        IL = 'IL',
        MY = 'MY',
        NZ = 'NZ',
        PT = 'PT',
        SG = 'SG',
        TH = 'TH',
        TW = 'TW',
        US = 'US',

    id = models.AutoField(primary_key=True,)

    country = models.CharField(name='country', max_length=2, choices=CountryCode.choices, default=CountryCode.TW,)
    province    = models.CharField(name='province', max_length=50,) # name of the province
    locality    = models.CharField(name='locality', max_length=50,) # name of the city or town
    street      = models.CharField(name='street',   max_length=50,) # name of the road, street, or lane
    # extra detail of the location, e.g. the name of the building, which floor, etc.
    # Note each record in this database table has to be mapped to a building of real world
    detail      = models.CharField(name='detail', max_length=100,)
    # if
    # floor =  0, that's basement B1 floor
    # floor = -1, that's basement B2 floor
    # floor = -2, that's basement B3 floor ... etc
    floor = models.SmallIntegerField(default=1, blank=True, null=True)
    # simple words to describe what you do in the location for your business
    description = models.CharField(name='description', blank=True, max_length=100,)

    def __str__(self):
        out = ["Nation:", None, ", city/town:", None,]
        out[1] = self.country
        out[3] = self.locality
        return "".join(out)

