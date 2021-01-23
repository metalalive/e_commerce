from django.db import models

class AbstractLocationApartment(models.Model):
    class Meta:
        abstract = True
    # metadata of each apartment can be attributes like :
    # * size of the entire apartment
    # * path of interior design diagram
    meta_path = models.CharField(max_length=200, unique=False)


class AbstractLocationRoom(models.Model):
    class Meta:
        abstract = True
    # metadata of each room can be attributes like :
    # * size of the entire room
    # * path of interior design diagram
    meta_path = models.CharField(max_length=200, unique=False)



