
from django.urls import  path
from ..views.productmgt import TrelloMemberProxyView, TrelloNotificationProxyView
from ..views.productmgt import ProductTagProxyView, ProductAttrTypeProxyView, FabricationIngredientProxyView
from ..views.productmgt import SaleableItemProxyView

app_name = 'productmgt'

urlpatterns = [
    path('trello/members/<slug:prof_id>',  TrelloMemberProxyView.as_view()  ),
    path('trello/notifications',  TrelloNotificationProxyView.as_view()  ),
    path('tags',               ProductTagProxyView.as_view()  ),
    path('tag/<slug:tag_id>',  ProductTagProxyView.as_view()  ),
    path('tag/<slug:tag_id>/ancestors',    ProductTagProxyView.as_view()  ),
    path('tag/<slug:tag_id>/descendants',  ProductTagProxyView.as_view()  ),
    path('attrtypes',              ProductAttrTypeProxyView.as_view() ,name='ProductAttrTypeProxyView' ),
    path('ingredients',                 FabricationIngredientProxyView.as_view() ,name='FabricationIngredientProxyView0' ),
    path('ingredient/<slug:ingre_id>',  FabricationIngredientProxyView.as_view() ,name='FabricationIngredientProxyView1' ),
    path('saleableitems',               SaleableItemProxyView.as_view()  ),
    path('saleableitem/<slug:item_id>', SaleableItemProxyView.as_view()  ),
]



