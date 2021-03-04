from django.urls import include, path, re_path
from ..views.productmgt import DashBoardView

app_name = 'productmgt'

urlpatterns = [
    path('dashboard',  DashBoardView.as_view()),
]


