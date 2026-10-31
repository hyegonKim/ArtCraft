from types import Optional

class FileInfo:
    size_in_bytes: int
    mime_type: str
    file_name: str
    
class BucketDetails:
    bucket_hash: str
    maybe_bucket_name: Optional[str]
    maybe_bucket_extension: Optional[str]
    
    
class MediaFileBucketDirectory: 
  object_hash: str
  directory: str
  optional_prefix: Optional[str]
  optional_prefix_extension: Optional[str]
  original_file_base_name: str
  
class WeightsType:
    directory: float
    
class FileOperationManager:
    def __init__(self, bucket,bucket_info:BucketDetails):
        self.bucket = bucket
        self.bucket_info = bucket_info

    def check_if_file_exists_in_db(self, file_token: str, callback) -> bool:
        pass
    
    
    
