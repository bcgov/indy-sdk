
python manage.py makemigrations api
python manage.py migrate
echo "from django.contrib.auth.models import User; User.objects.filter(username='wall-e').exists() or User.objects.create_superuser('wall-e', 'wall-e@example.com', 'pass1234')" | python manage.py shell
python manage.py runserver 0.0.0.0:8000

