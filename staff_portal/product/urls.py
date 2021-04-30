from django.urls import path

from .apps   import ProductConfig
from .views.base  import TagView, AttributeTypeView

urlpatterns = [
    path(ProductConfig.api_url[TagView.__name__][0] ,  TagView.as_view() ),
    path(ProductConfig.api_url[TagView.__name__][1] ,  TagView.as_view() ),
    path(ProductConfig.api_url[TagView.__name__][2] ,  TagView.as_view() ),
    path(ProductConfig.api_url[TagView.__name__][3] ,  TagView.as_view() ),
    path(ProductConfig.api_url[AttributeTypeView.__name__] ,  AttributeTypeView.as_view() ),
]

