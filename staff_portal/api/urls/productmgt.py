
from django.urls import  path
from ..views.productmgt import TrelloMemberProxyView, TrelloNotificationProxyView

app_name = 'productmgt'

urlpatterns = [
    path('trello/members/<slug:prof_id>',  TrelloMemberProxyView.as_view()  ),
    path('trello/notifications',  TrelloNotificationProxyView.as_view()  ),
]

