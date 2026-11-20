FROM python:3.13-slim
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1
WORKDIR /srv
COPY api/requirements.txt /srv/requirements.txt
RUN pip install --no-cache-dir -r requirements.txt
COPY api/ /srv/
COPY harness/ /harness/
ENV PYTHONPATH=/srv:/harness
