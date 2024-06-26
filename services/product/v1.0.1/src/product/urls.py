from django.urls import path

from .apps import ProductConfig
from .views.base import (
    TagView,
    TaggedSaleableView,
    AttributeTypeView,
    SaleableItemView,
    SaleablePackageView,
)
from .views.development import FabricationIngredientView

urlpatterns = [
    path(ProductConfig.api_url[TagView.__name__][0], TagView.as_view()),
    path(ProductConfig.api_url[TagView.__name__][1], TagView.as_view()),
    path(ProductConfig.api_url[TagView.__name__][2], TagView.as_view()),
    path(ProductConfig.api_url[TagView.__name__][3], TagView.as_view()),
    path(
        ProductConfig.api_url[TaggedSaleableView.__name__], TaggedSaleableView.as_view()
    ),
    path(
        ProductConfig.api_url[AttributeTypeView.__name__], AttributeTypeView.as_view()
    ),
    path(
        ProductConfig.api_url[FabricationIngredientView.__name__][0],
        FabricationIngredientView.as_view(),
    ),
    path(
        ProductConfig.api_url[FabricationIngredientView.__name__][1],
        FabricationIngredientView.as_view(),
    ),
    path(
        ProductConfig.api_url[SaleableItemView.__name__][0], SaleableItemView.as_view()
    ),
    path(
        ProductConfig.api_url[SaleableItemView.__name__][1], SaleableItemView.as_view()
    ),
    path(
        ProductConfig.api_url[SaleablePackageView.__name__][0],
        SaleablePackageView.as_view(),
    ),
    path(
        ProductConfig.api_url[SaleablePackageView.__name__][1],
        SaleablePackageView.as_view(),
    ),
]
