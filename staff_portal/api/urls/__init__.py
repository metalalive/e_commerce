from django.urls  import  path, re_path, include
from ..views.common   import LoginView, LogoutView
from ..views.security import UserActionHistoryAPIReadView, DynamicLoglevelAPIView

urlpatterns = [
    path('login',   LoginView.as_view()  ),
    path('logout',  LogoutView.as_view() ),
    path('activity_log',  UserActionHistoryAPIReadView.as_view() ),
    path('log_level',     DynamicLoglevelAPIView.as_view() ),
    path('usermgt/', include('api.urls.usermgt')),
    path('product/', include('api.urls.productmgt')),
    ##re_path(r'.*', Gateway.as_view()),
]

