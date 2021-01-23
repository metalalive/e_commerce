"""restaurant URL Configuration

The `urlpatterns` list routes URLs to views. For more information please see:
    https://docs.djangoproject.com/en/dev/topics/http/urls/
Examples:
Function views
    1. Add an import:  from my_app import views
    2. Add a URL to urlpatterns:  path('', views.home, name='home')
Class-based views
    1. Add an import:  from other_app.views import Home
    2. Add a URL to urlpatterns:  path('', Home.as_view(), name='home')
Including another URLconf
    1. Import the include() function: from django.urls import include, path
    2. Add a URL to urlpatterns:  path('blog/', include('blog.urls'))
"""
from django.urls import include, path

from .apps   import UserManagementConfig as UserMgtCfg
from common.views  import AsyncTaskResultView

urlpatterns = [
    path('{name}/'.format(name=UserMgtCfg.app_url),  include('user_management.urls')),
    #### path('product/',  include('product.urls')),
    #### path('location/', include('location.urls')),
    #### path('admin/', admin.site.urls), # TODO: restrict IP address for admin login

    path("api/asynctask/<slug:id>", AsyncTaskResultView.as_view()),
]

