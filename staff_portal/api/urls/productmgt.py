
from django.urls import  path
from ..views.productmgt import TrelloMemberProxyView, TrelloNotificationProxyView
from ..views.productmgt import ProductTagProxyView

app_name = 'productmgt'

urlpatterns = [
    path('trello/members/<slug:prof_id>',  TrelloMemberProxyView.as_view()  ),
    path('trello/notifications',  TrelloNotificationProxyView.as_view()  ),
    path('tags',               ProductTagProxyView.as_view()  ),
    path('tag/<slug:tag_id>',  ProductTagProxyView.as_view()  ),
    path('tag/<slug:tag_id>/ancestors',    ProductTagProxyView.as_view()  ),
    path('tag/<slug:tag_id>/descendants',  ProductTagProxyView.as_view()  ),
]

